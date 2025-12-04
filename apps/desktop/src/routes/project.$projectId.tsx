import { createFileRoute, Outlet, useNavigate } from "@tanstack/react-router";
import { createContext, useContext, useEffect, useState, useEffectEvent } from "react";
import { readProjectConfig } from "@/stores/projects";
import {
  addRecentProject,
  setProjectConfig,
  setProjectPath,
} from "@/stores/project-state";
import type { ProjectConfig } from "@/types/project";
import { Loader2Icon } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { TopBar } from "@/components/workspace/top-bar";
import { StatusBar } from "@/components/workspace/status-bar";

// Context to share project data with child routes
interface ProjectContextValue {
  config: ProjectConfig;
  projectPath: string;
  projectId: string;
  reloadConfig: () => void;
}

export const ProjectContext = createContext<ProjectContextValue | null>(null);

export function useProject() {
  const ctx = useContext(ProjectContext);
  if (!ctx) throw new Error("useProject must be used within ProjectContext");
  return ctx;
}

export const Route = createFileRoute("/project/$projectId")({
  component: ProjectLayout,
});

function ProjectLayout() {
  const { projectId } = Route.useParams();
  const navigate = useNavigate();
  const [config, setConfig] = useState<ProjectConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Decode project path from Base64
  const projectPath = atob(projectId);

  // useEffectEvent allows us to use latest values without being part of dependencies
  const loadProject = useEffectEvent(async (showToast = false) => {
    const doLoad = async () => {
      setLoading(true);
      setProjectPath(projectPath);
      const projectConfig = await readProjectConfig(projectPath);
      setConfig(projectConfig);
      setProjectConfig(projectConfig);

      addRecentProject({
        path: projectPath,
        name: projectConfig.name,
        lastOpened: new Date().toISOString(),
      });

      setError(null);
      setLoading(false);
    };

    if (showToast) {
      toast.promise(doLoad(), {
        loading: "Reloading project config...",
        success: "Project reloaded successfully",
        error: "Failed to reload project",
      });
    } else {
      try {
        await doLoad();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        setLoading(false);
      }
    }
  });

  // Initial load and file watcher setup
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    async function init() {
      await loadProject(false);

      try {
        await invoke("watch_project_config", { projectPath });
        unlisten = await listen("config-changed", async () => {
          await loadProject(true);
        });
      } catch (error) {
        console.error("Failed to start config watcher:", error);
      }
    }

    init();

    return () => {
      unlisten?.();
      invoke("unwatch_project_config").catch(console.error);
      invoke("close_project_connections", { projectPath }).catch(console.error);
    };
  }, [projectPath]);

  const handleConnectionChange = (connKey: string) => {
    navigate({ to: "/project/$projectId/conn/$connKey", params: { projectId, connKey } });
  };

  const handleReloadConfig = () => {
    loadProject(true);
  };

  if (loading) {
    return (
      <div className="flex flex-col h-screen">
        <header
          data-tauri-drag-region
          className="h-12 border-b bg-background/80 backdrop-blur-sm select-none"
        />
        <div className="flex-1 flex items-center justify-center">
          <div className="flex flex-col items-center gap-4">
            <Loader2Icon className="size-8 animate-spin text-muted-foreground" />
            <p className="text-sm text-muted-foreground">Loading project...</p>
          </div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col h-screen">
        <header
          data-tauri-drag-region
          className="h-12 border-b bg-background/80 backdrop-blur-sm select-none"
        />
        <div className="flex-1 flex items-center justify-center">
          <div className="flex flex-col items-center gap-4 max-w-md">
            <h2 className="text-lg font-semibold text-destructive">Failed to load project</h2>
            <p className="text-sm text-muted-foreground text-center">{error}</p>
            <p className="text-xs text-muted-foreground font-mono bg-muted p-2 rounded">
              {projectPath}
            </p>
          </div>
        </div>
      </div>
    );
  }

  if (!config) {
    return null;
  }

  const handleDisconnect = () => {
    navigate({ to: "/project/$projectId", params: { projectId } });
  };

  return (
    <ProjectContext.Provider value={{ config, projectPath, projectId, reloadConfig: handleReloadConfig }}>
      <div className="flex flex-col h-screen">
        <TopBar
          config={config}
          onReloadConfig={handleReloadConfig}
        />

        {/* Child routes render here */}
        <div className="flex-1 overflow-hidden">
          <Outlet />
        </div>

        <StatusBar
          onConnectionChange={handleConnectionChange}
          onDisconnect={handleDisconnect}
        />
      </div>
    </ProjectContext.Provider>
  );
}

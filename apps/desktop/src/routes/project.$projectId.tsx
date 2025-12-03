import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useState, useEffectEvent } from "react";
import { readProjectConfig } from "@/stores/projects";
import { addRecentProject, setActiveDatabase, setActiveEnvironment } from "@/stores/project-state";
import type { ProjectConfig } from "@/types/project";
import { Loader2Icon } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";

export const Route = createFileRoute("/project/$projectId")({
  component: ProjectWorkspace,
});

function ProjectWorkspace() {
  const { projectId } = Route.useParams();
  const [config, setConfig] = useState<ProjectConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Decode project path from Base64
  const projectPath = atob(projectId);

  // useEffectEvent allows us to use latest values without being part of dependencies
  const loadProject = useEffectEvent(async (showToast = false) => {
    const doLoad = async () => {
      setLoading(true);
      const projectConfig = await readProjectConfig(projectPath);
      setConfig(projectConfig);

      // Add to recent projects
      addRecentProject({
        path: projectPath,
        name: projectConfig.name,
        lastOpened: new Date().toISOString(),
      });

      // Set default environment and database
      setActiveEnvironment(projectConfig.environments.default);

      // Set first database as active if exists
      const firstDbKey = Object.keys(projectConfig.databases)[0];
      if (firstDbKey) {
        setActiveDatabase(firstDbKey);
      }

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
      // Initial load
      await loadProject(false);

      try {
        // Start file watcher
        await invoke("watch_project_config", {
          projectPath,
        });

        // Listen for config changes
        unlisten = await listen("config-changed", async () => {
          await loadProject(true);
        });
      } catch (error) {
        console.error("Failed to start config watcher:", error);
      }
    }

    init();

    // Cleanup
    return () => {
      if (unlisten) {
        unlisten();
      }
      invoke("unwatch_project_config").catch(console.error);
    };
  }, [projectPath]); // Only depends on projectPath, loadProject is stable via useEffectEvent

  if (loading) {
    return (
      <div className="flex items-center justify-center h-screen">
        <div className="flex flex-col items-center gap-4">
          <Loader2Icon className="size-8 animate-spin text-muted-foreground" />
          <p className="text-sm text-muted-foreground">Loading project...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-screen">
        <div className="flex flex-col items-center gap-4 max-w-md">
          <h2 className="text-lg font-semibold text-destructive">Failed to load project</h2>
          <p className="text-sm text-muted-foreground text-center">{error}</p>
          <p className="text-xs text-muted-foreground font-mono bg-muted p-2 rounded">
            {projectPath}
          </p>
        </div>
      </div>
    );
  }

  if (!config) {
    return null;
  }

  return (
    <div className="flex flex-col h-screen">
      <div className="border-b p-4">
        <h1 className="text-lg font-semibold">{config.name}</h1>
        <p className="text-sm text-muted-foreground">{config.description}</p>
      </div>

      <div className="flex-1 flex items-center justify-center">
        <div className="text-center space-y-4">
          <h2 className="text-2xl font-bold">Project Workspace</h2>
          <p className="text-muted-foreground">
            Database explorer and query editor coming soon...
          </p>
          <div className="text-sm text-muted-foreground space-y-1">
            <p>Version: {config.version}</p>
            <p>Databases: {Object.keys(config.databases).length}</p>
            <p>Environments: {config.environments.available.join(", ")}</p>
          </div>
        </div>
      </div>
    </div>
  );
}

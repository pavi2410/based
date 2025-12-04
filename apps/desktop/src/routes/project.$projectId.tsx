import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useState, useEffectEvent } from "react";
import { useStore } from "@nanostores/react";
import { readProjectConfig } from "@/stores/projects";
import {
  addRecentProject,
  setProjectConfig,
  setProjectPath,
  switchConnection,
  $activeConnection,
  $activeConnectionId,
  $connectionStatus,
} from "@/stores/project-state";
import type { ProjectConfig } from "@/types/project";
import { Loader2Icon } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable";
import { TopBar } from "@/components/workspace/top-bar";
import { WorkspaceSidebar } from "@/components/workspace/workspace-sidebar";
import { StatusBar } from "@/components/workspace/status-bar";

export const Route = createFileRoute("/project/$projectId")({
  component: ProjectWorkspace,
});

function ProjectWorkspace() {
  const { projectId } = Route.useParams();
  const [config, setConfig] = useState<ProjectConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Get reactive state from stores
  const activeConnection = useStore($activeConnection);
  const activeConnectionId = useStore($activeConnectionId);
  const connectionStatus = useStore($connectionStatus);

  // Decode project path from Base64
  const projectPath = atob(projectId);

  // Get active connection config
  const activeConnectionConfig =
    config && activeConnection ? config.connection[activeConnection] : null;

  // useEffectEvent allows us to use latest values without being part of dependencies
  const loadProject = useEffectEvent(async (showToast = false) => {
    const doLoad = async () => {
      setLoading(true);
      setProjectPath(projectPath); // Set project path for connection management
      const projectConfig = await readProjectConfig(projectPath);
      setConfig(projectConfig);
      setProjectConfig(projectConfig);

      // Add to recent projects
      addRecentProject({
        path: projectPath,
        name: projectConfig.name,
        lastOpened: new Date().toISOString(),
      });

      // Set first connection as active and connect
      const firstConnKey = Object.keys(projectConfig.connection)[0];
      if (firstConnKey) {
        // Use switchConnection to both set active and establish connection
        await switchConnection(firstConnKey);
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
      invoke("close_project_connections", { projectPath }).catch(console.error);
    };
  }, [projectPath]); // Only depends on projectPath, loadProject is stable via useEffectEvent

  // Handlers for selectors
  const handleConnectionChange = (connKey: string) => {
    switchConnection(connKey);
  };

  const handleReloadConfig = () => {
    loadProject(true);
  };

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
      {/* Top Bar */}
      <TopBar
        config={config}
        projectPath={projectPath}
        activeConnection={activeConnection}
        onConnectionChange={handleConnectionChange}
        onReloadConfig={handleReloadConfig}
      />

      {/* Main Content Area with Resizable Panels */}
      <ResizablePanelGroup direction="horizontal" className="flex-1">
        {/* Sidebar */}
        <ResizablePanel defaultSize={20} minSize={15} maxSize={40}>
          <WorkspaceSidebar
            activeConnection={activeConnection}
            connectionConfig={activeConnectionConfig}
            projectPath={projectPath}
          />
        </ResizablePanel>

        <ResizableHandle />

        {/* Main Content */}
        <ResizablePanel defaultSize={80}>
          <div className="flex items-center justify-center h-full">
            <div className="text-center space-y-4">
              <h2 className="text-2xl font-bold">Welcome to Your Project</h2>
              <p className="text-muted-foreground">
                Select a table or collection from the sidebar to explore
              </p>
              <div className="text-sm text-muted-foreground space-y-1">
                <p>Active Connection: {activeConnectionConfig?.label || activeConnection || "None"}</p>
                {activeConnectionId && (
                  <p className="text-xs font-mono opacity-50">ID: {activeConnectionId}</p>
                )}
              </div>
            </div>
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>

      {/* Status Bar */}
      <StatusBar
        activeConnection={activeConnection}
        connectionConfig={activeConnectionConfig}
        connectionStatus={connectionStatus}
      />
    </div>
  );
}

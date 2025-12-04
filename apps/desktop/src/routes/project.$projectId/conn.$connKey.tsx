import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { createContext, useContext, useEffect } from "react";
import { z } from "zod";
import { useStore } from "@nanostores/react";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { WorkspaceSidebar } from "@/components/workspace/workspace-sidebar";
import { DataViewer } from "@/components/workspace/data-viewer";
import { Loader2Icon } from "lucide-react";
import { ProjectContext } from "../project.$projectId";
import {
  switchConnection,
  disconnectConnection,
  $connection,
} from "@/stores/project-state";
import type { ConnectionConfig } from "@/types/project";

// Context to share connection state with child components
interface ConnectionContextValue {
  connKey: string;
  connectionConfig: ConnectionConfig;
  projectPath: string;
  selectedTable: string | undefined;
  selectedSchema: string | undefined;
  onSelectTable: (tableName: string, tableSchema?: string) => void;
}

export const ConnectionContext = createContext<ConnectionContextValue | null>(null);

export function useConnection() {
  const ctx = useContext(ConnectionContext);
  if (!ctx) throw new Error("useConnection must be used within ConnectionContext");
  return ctx;
}

// Search params schema for table selection
const searchSchema = z.object({
  table: z.string().optional(),
  schema: z.string().optional(),
});

export const Route = createFileRoute("/project/$projectId/conn/$connKey")({
  validateSearch: searchSchema,
  component: ConnectionLayout,
});

function ConnectionLayout() {
  const { connKey } = Route.useParams();
  const { table, schema } = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const ctx = useContext(ProjectContext);

  const { status: connectionStatus } = useStore($connection);

  // Derived values (safe even if ctx is null)
  const config = ctx?.config;
  const projectPath = ctx?.projectPath;
  const projectId = ctx?.projectId;
  const connectionConfig = config?.connection[connKey];

  // Connect when route is entered or connKey changes
  useEffect(() => {
    if (!ctx) return;
    switchConnection(connKey);
  }, [connKey, ctx]);

  // Context may not be ready during route transitions
  if (!ctx || !projectPath || !projectId) {
    return (
      <div className="flex items-center justify-center h-full">
        <Loader2Icon className="size-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  // Navigate to select a table (called by tree components)
  const handleSelectTable = (tableName: string, tableSchema?: string) => {
    navigate({
      search: (prev) => ({ ...prev, table: tableName, schema: tableSchema }),
    });
  };

  // Handle disconnect - navigate back to project root
  const handleDisconnect = async () => {
    await disconnectConnection();
    navigate({ to: "/project/$projectId", params: { projectId } });
  };

  // Show connecting state
  if (connectionStatus === "connecting") {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="flex flex-col items-center gap-3">
          <Loader2Icon className="size-5 animate-spin text-muted-foreground" />
          <p className="text-xs text-muted-foreground">Connecting...</p>
        </div>
      </div>
    );
  }

  // Connection not found in config
  if (!connectionConfig) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="flex flex-col items-center gap-2 max-w-sm">
          <h2 className="text-sm font-medium text-destructive">Connection not found</h2>
          <p className="text-xs text-muted-foreground text-center">
            "{connKey}" is not defined in the project config.
          </p>
        </div>
      </div>
    );
  }

  const connectionContextValue: ConnectionContextValue = {
    connKey,
    connectionConfig,
    projectPath,
    selectedTable: table,
    selectedSchema: schema,
    onSelectTable: handleSelectTable,
  };

  return (
    <ConnectionContext.Provider value={connectionContextValue}>
      <ResizablePanelGroup direction="horizontal" className="h-full">
        <ResizablePanel defaultSize={20} minSize={15} maxSize={40}>
          <WorkspaceSidebar
            onDisconnect={handleDisconnect}
          />
        </ResizablePanel>

        <ResizableHandle />

        <ResizablePanel defaultSize={80}>
          <DataViewer />
        </ResizablePanel>
      </ResizablePanelGroup>
    </ConnectionContext.Provider>
  );
}

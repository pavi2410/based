/**
 * Standalone DataViewer host for a pop-out table window.
 *
 * The detached window has no TanStack Router context (child windows
 * skip the router entirely), so we manually reconstruct the
 * `ConnectionContext` the browse stack expects:
 *  - Resolve the project config from disk via `read_project_config`.
 *  - Verify the connection key still exists.
 *  - Provide `connKey`, `connectionConfig`, `projectPath`, plus the
 *    selected table/schema from the `TabAddress`.
 *
 * This intentionally reuses the same `<DataViewer />` rendered in the
 * main window; any UX improvement lands in both places automatically.
 */
import { useQuery } from "@tanstack/react-query";
import { Loader2Icon } from "lucide-react";
import { cmd } from "@/commands";
import { DataViewer } from "@/components/workspace/data-viewer";
import { queryKeys } from "@/lib/query-keys";
import { ConnectionContext } from "@/routes/project.$projectId/conn.$connKey";
import type { TabAddress } from "@/bindings";

export function DetachedTableViewer({ address }: { address: TabAddress }) {
  // Only table addresses are currently detachable; the Inspector kind
  // would route to a schema inspector window (not yet built), and the
  // Query kind routes to the query editor. Fall out early if we get
  // something unexpected so the user sees a clear error.
  if (address.kind !== "table") {
    return (
      <div className="flex items-center justify-center h-screen text-xs text-muted-foreground">
        Unsupported tab kind: {address.kind}
      </div>
    );
  }
  const { connection, schema, name } = address;
  const projectPath = connection.project;
  const connKey = connection.conn_key;

  const configQuery = useQuery({
    queryKey: queryKeys.projectConfig(projectPath),
    queryFn: () => cmd.readProjectConfig(projectPath),
  });

  if (configQuery.status === "pending") {
    return (
      <div className="flex items-center justify-center h-screen">
        <Loader2Icon className="size-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (configQuery.status === "error") {
    return (
      <div className="flex items-center justify-center h-screen">
        <p className="text-xs text-destructive">
          Failed to load project config:{" "}
          {configQuery.error instanceof Error
            ? configQuery.error.message
            : String(configQuery.error)}
        </p>
      </div>
    );
  }

  const connectionConfig = configQuery.data.connection?.[connKey];
  if (!connectionConfig) {
    return (
      <div className="flex items-center justify-center h-screen">
        <p className="text-xs text-destructive">
          Connection "{connKey}" is no longer defined in this project.
        </p>
      </div>
    );
  }

  return (
    <ConnectionContext.Provider
      value={{
        connKey,
        connectionConfig,
        projectPath,
        selectedTable: name,
        selectedSchema: schema ?? undefined,
        // Pop-out windows don't switch tables; selection is frozen to
        // the one the window was opened for. Navigation stays in the
        // main window's tree.
        onSelectTable: () => {
          /* no-op in detached windows */
        },
      }}
    >
      <div className="flex flex-col h-screen">
        <div
          className="h-8 shrink-0 border-b bg-muted/30"
          data-tauri-drag-region
        />
        <div className="flex-1 min-h-0">
          <DataViewer />
        </div>
      </div>
    </ConnectionContext.Provider>
  );
}

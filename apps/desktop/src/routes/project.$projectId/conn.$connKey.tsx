import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { createContext, useContext, useEffect, useMemo } from "react";
import { z } from "zod";
import { useStore } from "@nanostores/react";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { WorkspaceSidebar } from "@/components/workspace/workspace-sidebar";
import { WorkspaceTabs } from "@/components/workspace/workspace-tabs";
import { DataViewer } from "@/components/workspace/data-viewer";
import { QueryEditor } from "@/components/workspace/query-editor";
import { Loader2Icon } from "lucide-react";
import { ProjectContext } from "../project.$projectId";
import {
  switchConnection,
  disconnectConnection,
  $connection,
} from "@/stores/project-state";
import type { ConnectionConfig } from "@/types/project";
import {
  $tabs,
  closeTab,
  savedQueryTabId,
  setActiveTab,
  tableTabId,
  upsertTab,
  type OpenTab,
  type TabScope,
} from "@/stores/tabs-store";

// Context to share connection state with child components
interface ConnectionContextValue {
  connKey: string;
  connectionConfig: ConnectionConfig;
  projectPath: string;
  selectedTable: string | undefined;
  selectedSchema: string | undefined;
  onSelectTable: (tableName: string, tableSchema?: string) => void;
}

export const ConnectionContext = createContext<ConnectionContextValue | null>(
  null,
);

export function useConnection() {
  const ctx = useContext(ConnectionContext);
  if (!ctx)
    throw new Error("useConnection must be used within ConnectionContext");
  return ctx;
}

// Search params schema for table/query selection
const searchSchema = z.object({
  table: z.string().optional(),
  schema: z.string().optional(),
  query: z.string().optional(), // Query filename
  newQuery: z.boolean().optional(), // Creating new query
});

export const Route = createFileRoute("/project/$projectId/conn/$connKey")({
  validateSearch: searchSchema,
  component: ConnectionLayout,
});

function ConnectionLayout() {
  const { connKey } = Route.useParams();
  const { table, schema, query, newQuery } = Route.useSearch();
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

  // Keep tabs store in sync with the URL. The URL stays the source of
  // truth for _what_ is visible; the tabs store is the source of truth
  // for _what's still open_. Upserting on every URL change means we
  // get a tab for every table/query the user lands on, including
  // directly navigating via deep links.
  const scope: TabScope | null = useMemo(
    () => (ctx?.projectPath ? { projectPath: ctx.projectPath, connKey } : null),
    [ctx?.projectPath, connKey],
  );
  useEffect(() => {
    if (!scope) return;
    if (table) {
      const id = tableTabId(schema, table);
      upsertTab({
        id,
        kind: "table",
        title: schema ? `${schema}.${table}` : table,
        scope,
        table: { schema: schema ?? null, name: table },
      });
      setActiveTab(scope, id);
    } else if (query) {
      const id = savedQueryTabId(query);
      upsertTab({
        id,
        kind: "query",
        title: query,
        scope,
        queryFilename: query,
      });
      setActiveTab(scope, id);
    } else if (newQuery) {
      // A new draft always gets its own tab id so the user can have
      // multiple unsaved queries at once. We stash it in the route
      // state via a synthesized id keyed by the current timestamp;
      // once saved, the filename tab supersedes it.
      const id = "query:new:current";
      upsertTab({
        id,
        kind: "query",
        title: "New query",
        scope,
        isNewQuery: true,
      });
      setActiveTab(scope, id);
    } else {
      setActiveTab(scope, null);
    }
  }, [scope, table, schema, query, newQuery]);

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
      search: { table: tableName, schema: tableSchema },
    });
  };

  // Navigate to select a query
  const handleSelectQuery = (filename: string) => {
    navigate({
      search: { query: filename },
    });
  };

  // Navigate to create new query
  const handleNewQuery = () => {
    navigate({
      search: { newQuery: true },
    });
  };

  // Close query editor
  const handleCloseQuery = () => {
    navigate({
      search: {},
    });
  };

  // Global keyboard shortcuts for tabs. Scoped to the document and
  // filtered so we don't hijack text input shortcuts.
  useEffect(() => {
    if (!scope) return;
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const isEditable =
        target?.tagName === "INPUT" ||
        target?.tagName === "TEXTAREA" ||
        target?.isContentEditable;
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;

      if (e.key === "t" || e.key === "T") {
        e.preventDefault();
        navigate({ search: { newQuery: true } });
      } else if ((e.key === "w" || e.key === "W") && !isEditable) {
        e.preventDefault();
        const currentId = table
          ? tableTabId(schema, table)
          : query
            ? savedQueryTabId(query)
            : newQuery
              ? "query:new:current"
              : null;
        if (!currentId) return;
        const nextId = closeTab(scope, currentId);
        if (!nextId) {
          navigate({ search: {} });
          return;
        }
        const next = $tabs.get().find((t) => t.id === nextId);
        if (next) {
          if (next.kind === "table" && next.table) {
            navigate({
              search: {
                table: next.table.name,
                schema: next.table.schema ?? undefined,
              },
            });
          } else if (next.kind === "query") {
            if (next.isNewQuery) navigate({ search: { newQuery: true } });
            else if (next.queryFilename)
              navigate({ search: { query: next.queryFilename } });
          }
        }
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [scope, table, schema, query, newQuery, navigate]);

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
          <h2 className="text-sm font-medium text-destructive">
            Connection not found
          </h2>
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

  const handleActivateTab = (tab: OpenTab) => {
    if (tab.kind === "table" && tab.table) {
      navigate({
        search: {
          table: tab.table.name,
          schema: tab.table.schema ?? undefined,
        },
      });
    } else if (tab.kind === "query") {
      if (tab.isNewQuery) {
        navigate({ search: { newQuery: true } });
      } else if (tab.queryFilename) {
        navigate({ search: { query: tab.queryFilename } });
      }
    }
  };

  // After a tab closes, navigate to the URL of the remaining active
  // tab (or clear to an empty workspace if none remain).
  const handleTabClosed = (_tab: OpenTab, nextActiveId: string | null) => {
    if (!nextActiveId) {
      navigate({ search: {} });
      return;
    }
    const next = $tabs
      .get()
      .find((t) => t.id === nextActiveId && t.scope.connKey === connKey);
    if (next) handleActivateTab(next);
  };

  return (
    <ConnectionContext.Provider value={connectionContextValue}>
      <ResizablePanelGroup direction="horizontal" className="h-full">
        <ResizablePanel defaultSize={20} minSize={15} maxSize={40}>
          <WorkspaceSidebar
            onDisconnect={handleDisconnect}
            onSelectQuery={handleSelectQuery}
            onNewQuery={handleNewQuery}
            selectedQuery={query}
          />
        </ResizablePanel>

        <ResizableHandle />

        <ResizablePanel defaultSize={80}>
          <div className="flex flex-col h-full">
            {scope ? (
              <WorkspaceTabs
                scope={scope}
                onActivate={handleActivateTab}
                onClose={handleTabClosed}
                onNewQuery={handleNewQuery}
              />
            ) : null}
            <div className="flex-1 min-h-0">
              {query || newQuery ? (
                <QueryEditor
                  projectPath={projectPath}
                  connectionKey={connKey}
                  engine={connectionConfig.engine}
                  filename={query}
                  onClose={handleCloseQuery}
                  onSaved={handleSelectQuery}
                />
              ) : table ? (
                <DataViewer />
              ) : (
                <EmptyWorkspace onNewQuery={handleNewQuery} />
              )}
            </div>
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </ConnectionContext.Provider>
  );
}

function EmptyWorkspace({ onNewQuery }: { onNewQuery: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center h-full gap-3 text-center px-6">
      <p className="text-sm text-muted-foreground">
        Pick a table from the sidebar or open a query to get started.
      </p>
      <button
        type="button"
        onClick={onNewQuery}
        className="text-xs underline text-muted-foreground hover:text-foreground"
      >
        New query
      </button>
    </div>
  );
}

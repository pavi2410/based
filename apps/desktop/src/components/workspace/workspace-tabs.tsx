/**
 * WorkspaceTabs — the horizontal tab strip that lives above the main
 * editor/data panel of a connection.
 *
 * Responsibilities:
 *   - Render open tabs for the (project, connection) scope.
 *   - Dispatch navigation when a tab is clicked so the URL remains
 *     the source of truth.
 *   - Support middle-click and X button to close.
 *   - Support drag-to-detach: dragging a tab vertically out of the
 *     strip spawns a pop-out window via the existing WindowManager
 *     and removes the tab from the local list.
 *
 * Visual notes:
 *   - Compact by design (24px tall) to match the DataGrip-density
 *     target; the whole strip should never feel chunky.
 */
import { useStore } from "@nanostores/react";
import {
  DatabaseIcon,
  ExternalLinkIcon,
  FileTextIcon,
  PlusIcon,
  TableIcon,
  XIcon,
} from "lucide-react";
import { useCallback, useMemo, useRef } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { useWindow } from "@/hooks/use-window";
import { cn } from "@/lib/utils";
import {
  $activeByScope,
  $tabs,
  closeTab,
  type OpenTab,
  type TabScope,
} from "@/stores/tabs-store";

export interface WorkspaceTabsProps {
  scope: TabScope;
  onActivate: (tab: OpenTab) => void;
  onClose: (tab: OpenTab, nextActiveId: string | null) => void;
  onNewQuery: () => void;
}

export function WorkspaceTabs({
  scope,
  onActivate,
  onClose,
  onNewQuery,
}: WorkspaceTabsProps) {
  const allTabs = useStore($tabs);
  const activeByScope = useStore($activeByScope);
  const { openTab } = useWindow();

  const tabs = useMemo(
    () =>
      allTabs.filter(
        (t) =>
          t.scope.projectPath === scope.projectPath &&
          t.scope.connKey === scope.connKey,
      ),
    [allTabs, scope.projectPath, scope.connKey],
  );
  const activeId = activeByScope[`${scope.projectPath}::${scope.connKey}`];

  const barRef = useRef<HTMLDivElement>(null);

  const handleClose = useCallback(
    (tab: OpenTab) => {
      const nextId = closeTab(scope, tab.id);
      onClose(tab, nextId);
    },
    [scope, onClose],
  );

  const handlePopOut = useCallback(
    async (tab: OpenTab) => {
      if (tab.kind !== "table" || !tab.table) {
        // Query pop-out is not implemented yet; detached query tabs
        // need state sync with file watchers. Bail early rather than
        // open a blank window.
        toast.message("Detaching queries isn't supported yet.");
        return;
      }
      try {
        await openTab({
          kind: "table",
          connection: {
            project: scope.projectPath,
            conn_key: scope.connKey,
          },
          schema: tab.table.schema,
          name: tab.table.name,
        });
        // Tab stays open in the main window on purpose — this matches
        // DataGrip where "New Window" mirrors rather than moves.
      } catch (e) {
        toast.error(
          e instanceof Error ? `Couldn't pop out: ${e.message}` : String(e),
        );
      }
    },
    [openTab, scope.projectPath, scope.connKey],
  );

  const handleDragEnd = useCallback(
    (tab: OpenTab, e: React.DragEvent<HTMLDivElement>) => {
      // Detach heuristic: if the drop ended more than ~40px below the
      // tab strip, treat it as "dragged out" and spawn a window.
      const rect = barRef.current?.getBoundingClientRect();
      if (!rect) return;
      const distance = e.clientY - rect.bottom;
      if (distance > 40 || e.clientY < rect.top - 40) {
        void handlePopOut(tab);
      }
    },
    [handlePopOut],
  );

  return (
    <div
      ref={barRef}
      className="flex items-stretch h-7 shrink-0 border-b bg-muted/30 overflow-x-auto"
      role="tablist"
    >
      {tabs.length === 0 ? (
        <div className="flex items-center px-3 text-xs text-muted-foreground">
          No open tabs
        </div>
      ) : (
        tabs.map((tab) => {
          const isActive = tab.id === activeId;
          return (
            <div
              key={tab.id}
              role="tab"
              aria-selected={isActive}
              draggable
              onDragEnd={(e) => handleDragEnd(tab, e)}
              onMouseDown={(e) => {
                // Middle-click closes.
                if (e.button === 1) {
                  e.preventDefault();
                  handleClose(tab);
                }
              }}
              onClick={() => onActivate(tab)}
              className={cn(
                "group flex items-center gap-1.5 px-2 text-xs cursor-pointer select-none border-r",
                "min-w-[120px] max-w-[220px]",
                isActive
                  ? "bg-background text-foreground"
                  : "text-muted-foreground hover:bg-background/70",
              )}
              title={tab.title}
            >
              <TabIcon tab={tab} />
              <span className="flex-1 truncate">{tab.title}</span>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  handleClose(tab);
                }}
                className="opacity-0 group-hover:opacity-100 hover:text-foreground rounded-sm p-0.5"
                aria-label={`Close ${tab.title}`}
              >
                <XIcon className="size-3" />
              </button>
              {tab.kind === "table" ? (
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    void handlePopOut(tab);
                  }}
                  className="opacity-0 group-hover:opacity-100 hover:text-foreground rounded-sm p-0.5"
                  aria-label={`Pop out ${tab.title}`}
                  title="Detach to new window"
                >
                  <ExternalLinkIcon className="size-3" />
                </button>
              ) : null}
            </div>
          );
        })
      )}
      <Button
        variant="ghost"
        size="icon"
        className="size-7 rounded-none shrink-0"
        onClick={onNewQuery}
        title="New query"
        aria-label="New query"
      >
        <PlusIcon className="size-3.5" />
      </Button>
    </div>
  );
}

function TabIcon({ tab }: { tab: OpenTab }) {
  if (tab.kind === "query") {
    return <FileTextIcon className="size-3 shrink-0" />;
  }
  if (tab.kind === "table") {
    return <TableIcon className="size-3 shrink-0" />;
  }
  return <DatabaseIcon className="size-3 shrink-0" />;
}

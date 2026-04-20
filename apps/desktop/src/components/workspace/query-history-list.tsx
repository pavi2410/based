/**
 * QueryHistoryList — sidebar panel that shows previously-executed
 * queries for the current (project, conn). Click to load into the
 * editor; the clear button wipes history scoped to the current
 * connection only.
 *
 * No virtualization: the list is bounded to 200 entries by
 * `query-history-store` so a vanilla DOM list is more than fast
 * enough.
 */
import { useStore } from "@nanostores/react";
import { formatDistanceToNowStrict } from "date-fns";
import { Trash2Icon } from "lucide-react";
import { useMemo } from "react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  $queryHistory,
  clearHistory,
  type HistoryEntry,
} from "@/stores/query-history-store";

export interface QueryHistoryListProps {
  projectPath: string;
  connKey: string;
  onSelect: (entry: HistoryEntry) => void;
}

export function QueryHistoryList({
  projectPath,
  connKey,
  onSelect,
}: QueryHistoryListProps) {
  const all = useStore($queryHistory);
  const entries = useMemo(
    () =>
      all.filter((e) => e.projectPath === projectPath && e.connKey === connKey),
    [all, projectPath, connKey],
  );

  if (entries.length === 0) {
    return (
      <div className="flex items-center justify-center h-32 text-xs text-muted-foreground px-4 text-center">
        No queries run yet. Execute a query to see it here.
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between px-3 py-1.5 border-b">
        <span className="text-xs font-medium">History</span>
        <Button
          variant="ghost"
          size="icon"
          className="size-6"
          onClick={() => clearHistory({ projectPath, connKey })}
          title="Clear history for this connection"
        >
          <Trash2Icon className="size-3.5" />
        </Button>
      </div>
      <ScrollArea className="flex-1">
        <ul className="py-1">
          {entries.map((entry) => (
            <li key={entry.id}>
              <button
                type="button"
                onClick={() => onSelect(entry)}
                className="w-full text-left px-3 py-2 hover:bg-muted/50 border-b border-border/30 flex flex-col gap-0.5"
              >
                <code className="text-xs truncate font-mono">
                  {entry.query.slice(0, 80).replace(/\s+/g, " ")}
                </code>
                <div className="flex items-center gap-2 text-[10px] text-muted-foreground">
                  <span>
                    {formatDistanceToNowStrict(entry.ranAt, {
                      addSuffix: true,
                    })}
                  </span>
                  {entry.durationMs != null ? (
                    <span>· {entry.durationMs}ms</span>
                  ) : (
                    <span className="text-destructive">· error</span>
                  )}
                  {entry.rowCount != null ? (
                    <span>· {entry.rowCount.toLocaleString()} rows</span>
                  ) : null}
                </div>
              </button>
            </li>
          ))}
        </ul>
      </ScrollArea>
    </div>
  );
}

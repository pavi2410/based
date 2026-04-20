/**
 * Header strip for the DataViewer — title, view toggle, action
 * buttons (new row, export, undo, pop-out, refresh).
 *
 * Extracted from the monolithic DataViewer so the viewer can stay
 * focused on data flow. The header is deliberately a "dumb" component:
 * every action is a callback from the parent, which owns the
 * corresponding state (undo stack, mutations hook, pop-out plumbing).
 */
import {
  DownloadIcon,
  ExternalLinkIcon,
  PlusIcon,
  RefreshCwIcon,
  TableIcon,
  UndoIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { type TableView, ViewToggle } from "@/components/workspace/view-toggle";

export interface DataViewerHeaderProps {
  selectedTable: string;
  selectedSchema: string | undefined;
  totalCount: number;
  view: TableView;
  onViewChange: (v: TableView) => void;
  onNewRow: () => void;
  canInsert: boolean;
  onExportCsv: () => void;
  onExportJson: () => void;
  canExport: boolean;
  onUndo: () => void;
  canUndo: boolean;
  undoLabel: string | undefined;
  onPopOut: (() => void) | null;
  onRefresh: () => void;
  isRefreshing: boolean;
}

export function DataViewerHeader(props: DataViewerHeaderProps) {
  const {
    selectedTable,
    selectedSchema,
    totalCount,
    view,
    onViewChange,
    onNewRow,
    canInsert,
    onExportCsv,
    onExportJson,
    canExport,
    onUndo,
    canUndo,
    undoLabel,
    onPopOut,
    onRefresh,
    isRefreshing,
  } = props;

  return (
    <div className="flex items-center justify-between px-3 py-1.5 border-b bg-muted/20">
      <div className="flex items-center gap-1.5">
        <TableIcon className="size-3.5 text-muted-foreground" />
        <h2 className="text-sm font-medium">
          {selectedSchema ? `${selectedSchema}.` : ""}
          {selectedTable}
        </h2>
      </div>
      <div className="flex items-center gap-1">
        <ViewToggle view={view} onChange={onViewChange} />
        <span className="text-xs text-muted-foreground tabular-nums ml-2">
          {totalCount.toLocaleString()} rows
        </span>
        <Button
          variant="ghost"
          size="sm"
          className="h-6 px-2 text-[11px]"
          onClick={onNewRow}
          disabled={!canInsert}
          title="Insert new row"
        >
          <PlusIcon className="size-3 mr-1" />
          New
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <Button
                variant="ghost"
                size="sm"
                className="h-6 px-2 text-[11px]"
                title="Export current page"
                disabled={!canExport}
              >
                <DownloadIcon className="size-3 mr-1" />
                Export
              </Button>
            }
          />
          <DropdownMenuContent align="end" className="text-xs">
            <DropdownMenuItem className="text-xs" onClick={onExportCsv}>
              Download CSV (current page)
            </DropdownMenuItem>
            <DropdownMenuItem className="text-xs" onClick={onExportJson}>
              Download JSON (current page)
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <Button
          variant="ghost"
          size="icon"
          className="size-6"
          onClick={onUndo}
          disabled={!canUndo}
          title={canUndo ? `Undo: ${undoLabel}` : "Nothing to undo"}
        >
          <UndoIcon className="size-3.5" />
        </Button>
        {onPopOut ? (
          <Button
            variant="ghost"
            size="icon"
            className="size-6"
            onClick={onPopOut}
            title="Open in new window"
          >
            <ExternalLinkIcon className="size-3.5" />
          </Button>
        ) : null}
        <Button
          variant="ghost"
          size="icon"
          className="size-6"
          onClick={onRefresh}
          disabled={isRefreshing}
        >
          <RefreshCwIcon
            className={`size-3.5 ${isRefreshing ? "animate-spin" : ""}`}
          />
        </Button>
      </div>
    </div>
  );
}

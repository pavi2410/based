/**
 * Segmented Data / Structure toggle rendered inside the DataViewer
 * header. Extracted so the viewer file stays focused on data flow
 * instead of inline presentation, and so the toggle is reusable in a
 * future detached Inspector window.
 *
 * Mirrors the Data/Structure tabs you'd expect from DataGrip / TablePlus
 * but kept inside the viewer for now so we don't fight the outer
 * workspace router yet (see Phase 2 tabs todo for the real tabbed
 * workspace).
 */
import { DatabaseIcon, TableIcon } from "lucide-react";

export type TableView = "data" | "structure";

export function ViewToggle({
  view,
  onChange,
}: {
  view: TableView;
  onChange: (v: TableView) => void;
}) {
  return (
    <div
      role="tablist"
      aria-label="Table view"
      className="inline-flex items-center rounded-md border bg-background p-0.5"
    >
      <button
        type="button"
        role="tab"
        aria-selected={view === "data"}
        onClick={() => onChange("data")}
        className={`flex items-center gap-1 px-2 h-5 text-[11px] rounded-sm transition-colors ${
          view === "data"
            ? "bg-muted text-foreground"
            : "text-muted-foreground hover:text-foreground"
        }`}
      >
        <TableIcon className="size-3" />
        Data
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={view === "structure"}
        onClick={() => onChange("structure")}
        className={`flex items-center gap-1 px-2 h-5 text-[11px] rounded-sm transition-colors ${
          view === "structure"
            ? "bg-muted text-foreground"
            : "text-muted-foreground hover:text-foreground"
        }`}
      >
        <DatabaseIcon className="size-3" />
        Structure
      </button>
    </div>
  );
}

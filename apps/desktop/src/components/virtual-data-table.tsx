/**
 * VirtualDataTable — a virtualized variant of `DataTable` for the
 * browse grid.
 *
 * Why a new component instead of folding virtualization into
 * `DataTable`:
 *  - The query-results grid in `query-editor` is short-lived and
 *    doesn't benefit from virtualization; keeping a simple
 *    `DataTable` there avoids the extra layout quirks that come with
 *    `position: absolute` + `transform` row positioning.
 *  - Virtualization changes the DOM structure (one scroll container
 *    + absolutely-positioned rows), so it's easier to keep the
 *    virtualized markup explicit than to branch on a prop.
 *
 * Features:
 *  - Row virtualization via `@tanstack/react-virtual`, ~28px rows.
 *  - Multi-column sort via shift-click (toggle `enableMultiSort`).
 *  - Column resizing (drag the right edge of a header).
 *  - Optional row context menu (same API as `DataTable`).
 *  - Optional cell-click callback, used by the parent DataViewer to
 *    open the cell-detail panel.
 */
import {
  type ColumnDef,
  type SortingState,
  type OnChangeFn,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useRef } from "react";
import { ArrowUpIcon, ArrowDownIcon, ArrowUpDownIcon } from "lucide-react";
import { ContextMenu, ContextMenuTrigger } from "@/components/ui/context-menu";

interface Props<TData, TValue> {
  columns: ColumnDef<TData, TValue>[];
  data: TData[];
  sorting?: SortingState;
  onSortingChange?: OnChangeFn<SortingState>;
  renderRowContextMenu?: (row: TData) => React.ReactNode;
  onCellClick?: (args: {
    row: TData;
    columnId: string;
    value: unknown;
  }) => void;
}

const ROW_HEIGHT = 28; // matches the compact density baseline (--row-height)

export function VirtualDataTable<TData, TValue>({
  columns,
  data,
  sorting,
  onSortingChange,
  renderRowContextMenu,
  onCellClick,
}: Props<TData, TValue>) {
  const table = useReactTable({
    data,
    columns,
    getCoreRowModel: getCoreRowModel(),
    manualSorting: true,
    enableMultiSort: true,
    isMultiSortEvent: (e) => (e as unknown as MouseEvent).shiftKey === true,
    columnResizeMode: "onChange",
    state: { sorting },
    onSortingChange,
    defaultColumn: {
      minSize: 60,
      size: 160,
      maxSize: 800,
    },
  });

  const scrollRef = useRef<HTMLDivElement>(null);
  const { rows } = table.getRowModel();

  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 10,
  });

  const totalWidth = table.getTotalSize();

  return (
    <div
      ref={scrollRef}
      className="relative h-full overflow-auto text-xs"
      style={{ contain: "strict" }}
    >
      {/* Header: sticky, width matches total column size */}
      <div
        className="sticky top-0 z-10 bg-muted/40 border-b"
        style={{ width: totalWidth }}
      >
        {table.getHeaderGroups().map((headerGroup) => (
          <div
            key={headerGroup.id}
            className="flex"
            style={{ height: ROW_HEIGHT }}
          >
            {headerGroup.headers.map((header) => (
              <div
                key={header.id}
                className="relative flex items-center h-full px-2 border-r text-xs font-medium select-none"
                style={{ width: header.getSize() }}
              >
                {header.isPlaceholder ? null : (
                  <button
                    type="button"
                    className="flex items-center gap-1 hover:text-foreground transition-colors cursor-pointer truncate"
                    onClick={(e) =>
                      header.column.getToggleSortingHandler()?.(e)
                    }
                    title={
                      typeof header.column.columnDef.header === "string"
                        ? header.column.columnDef.header
                        : undefined
                    }
                  >
                    <span className="truncate">
                      {flexRender(
                        header.column.columnDef.header,
                        header.getContext(),
                      )}
                    </span>
                    {header.column.getIsSorted() === "asc" ? (
                      <ArrowUpIcon className="size-3 shrink-0" />
                    ) : header.column.getIsSorted() === "desc" ? (
                      <ArrowDownIcon className="size-3 shrink-0" />
                    ) : (
                      <ArrowUpDownIcon className="size-3 opacity-30 shrink-0" />
                    )}
                  </button>
                )}
                {header.column.getCanResize() ? (
                  <div
                    onMouseDown={header.getResizeHandler()}
                    onTouchStart={header.getResizeHandler()}
                    className={`absolute right-0 top-0 h-full w-1 cursor-col-resize select-none touch-none hover:bg-primary/40 ${
                      header.column.getIsResizing() ? "bg-primary" : ""
                    }`}
                    role="separator"
                    aria-orientation="vertical"
                  />
                ) : null}
              </div>
            ))}
          </div>
        ))}
      </div>

      {/* Body: absolutely-positioned virtualized rows */}
      {rows.length === 0 ? (
        <div className="flex items-center justify-center h-16 text-muted-foreground">
          No results
        </div>
      ) : (
        <div
          className="relative"
          style={{
            height: rowVirtualizer.getTotalSize(),
            width: totalWidth,
          }}
        >
          {rowVirtualizer.getVirtualItems().map((virtualRow) => {
            const row = rows[virtualRow.index];
            const rowEl = (
              <div
                data-index={virtualRow.index}
                ref={rowVirtualizer.measureElement}
                className="absolute left-0 top-0 flex border-b hover:bg-muted/30"
                style={{
                  height: ROW_HEIGHT,
                  transform: `translateY(${virtualRow.start}px)`,
                  width: totalWidth,
                }}
              >
                {row.getVisibleCells().map((cell) => (
                  <div
                    key={cell.id}
                    className="flex items-center h-full px-2 border-r text-nowrap font-mono tabular-nums overflow-hidden cursor-default"
                    style={{ width: cell.column.getSize() }}
                    onClick={() =>
                      onCellClick?.({
                        row: row.original,
                        columnId: cell.column.id,
                        value: cell.getValue(),
                      })
                    }
                  >
                    <div className="truncate w-full">
                      {flexRender(
                        cell.column.columnDef.cell,
                        cell.getContext(),
                      )}
                    </div>
                  </div>
                ))}
              </div>
            );

            if (!renderRowContextMenu) {
              return <div key={row.id}>{rowEl}</div>;
            }
            return (
              <ContextMenu key={row.id}>
                <ContextMenuTrigger asChild>{rowEl}</ContextMenuTrigger>
                {renderRowContextMenu(row.original)}
              </ContextMenu>
            );
          })}
        </div>
      )}
    </div>
  );
}

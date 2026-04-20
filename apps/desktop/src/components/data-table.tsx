import {
  type ColumnDef,
  type SortingState,
  type OnChangeFn,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { ArrowUpIcon, ArrowDownIcon, ArrowUpDownIcon } from "lucide-react";

import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ContextMenu, ContextMenuTrigger } from "@/components/ui/context-menu";

interface DataTableProps<TData, TValue> {
  columns: ColumnDef<TData, TValue>[];
  data: TData[];
  sorting?: SortingState;
  onSortingChange?: OnChangeFn<SortingState>;
  /**
   * Optional row-level context menu content. When provided, each
   * `<TableRow>` is wrapped in a `<ContextMenu>`; the render prop
   * receives the row's original data so the caller can decide what
   * actions to offer (e.g. Edit / Duplicate / Delete) per row.
   */
  renderRowContextMenu?: (row: TData) => React.ReactNode;
}

export function DataTable<TData, TValue>({
  columns,
  data,
  sorting,
  onSortingChange,
  renderRowContextMenu,
}: DataTableProps<TData, TValue>) {
  const table = useReactTable({
    data,
    columns,
    getCoreRowModel: getCoreRowModel(),
    manualSorting: true,
    state: {
      sorting,
    },
    onSortingChange,
  });

  return (
    <Table>
      <TableHeader className="bg-muted/40 sticky top-0 z-10">
        {table.getHeaderGroups().map((headerGroup) => (
          <TableRow key={headerGroup.id} className="hover:bg-transparent">
            {headerGroup.headers.map((header) => (
              <TableHead
                key={header.id}
                className="h-8 px-2 text-xs border-b border-r last:border-r-0"
              >
                {header.isPlaceholder ? null : (
                  <button
                    type="button"
                    className="flex items-center gap-1 hover:text-foreground transition-colors cursor-pointer select-none"
                    onClick={() => {
                      const isSorted = header.column.getIsSorted();
                      if (isSorted === "asc") {
                        header.column.toggleSorting(true);
                      } else if (isSorted === "desc") {
                        header.column.clearSorting();
                      } else {
                        header.column.toggleSorting(false);
                      }
                    }}
                  >
                    {flexRender(
                      header.column.columnDef.header,
                      header.getContext(),
                    )}
                    {header.column.getIsSorted() === "asc" ? (
                      <ArrowUpIcon className="size-3" />
                    ) : header.column.getIsSorted() === "desc" ? (
                      <ArrowDownIcon className="size-3" />
                    ) : (
                      <ArrowUpDownIcon className="size-3 opacity-30" />
                    )}
                  </button>
                )}
              </TableHead>
            ))}
          </TableRow>
        ))}
      </TableHeader>
      <TableBody>
        {table.getRowModel().rows?.length ? (
          table.getRowModel().rows.map((row) => {
            const rowEl = (
              <TableRow
                key={row.id}
                data-state={row.getIsSelected() && "selected"}
                className="hover:bg-muted/30"
              >
                {row.getVisibleCells().map((cell) => (
                  <TableCell
                    key={cell.id}
                    className="h-7 px-2 py-0 text-xs text-nowrap border-r last:border-r-0 tabular-nums font-mono"
                  >
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </TableCell>
                ))}
              </TableRow>
            );
            if (!renderRowContextMenu) return rowEl;
            return (
              <ContextMenu key={row.id}>
                <ContextMenuTrigger asChild>{rowEl}</ContextMenuTrigger>
                {renderRowContextMenu(row.original)}
              </ContextMenu>
            );
          })
        ) : (
          <TableRow>
            <TableCell
              colSpan={columns.length}
              className="h-16 text-center text-muted-foreground"
            >
              No results
            </TableCell>
          </TableRow>
        )}
      </TableBody>
    </Table>
  );
}

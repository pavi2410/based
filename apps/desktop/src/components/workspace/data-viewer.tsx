import { useQuery } from "@tanstack/react-query";
import { useStore } from "@nanostores/react";
import { cmd, type BrowseOptions } from "@/commands";
import { useState, useEffect, useMemo, useCallback } from "react";
import {
  Loader2Icon,
  RefreshCwIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  ChevronsLeftIcon,
  ChevronsRightIcon,
  TableIcon,
  DatabaseIcon,
  PencilIcon,
  TrashIcon,
  CopyIcon,
  PlusIcon,
  UndoIcon,
} from "lucide-react";
import { toast } from "sonner";
import { SchemaInspector } from "@/components/workspace/schema-inspector";
import {
  RowEditorDialog,
  type EditorMode,
} from "@/components/workspace/row-editor-dialog";
import {
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
} from "@/components/ui/context-menu";
import { useRowMutations, type RowMap } from "@/hooks/use-row-mutations";
import { $undoStack } from "@/stores/row-mutations-store";
import type { TableDescription } from "@/types/project";
import { Button } from "@/components/ui/button";
import { DataTable } from "@/components/data-table";
import { useConnection } from "@/routes/project.$projectId/conn.$connKey";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { ColumnDef, SortingState } from "@tanstack/react-table";
import { DataTableFilter } from "@/components/data-table-filter/components/data-table-filter";
import { useDataTableFilters } from "@/components/data-table-filter/hooks/use-data-table-filters";
import type {
  ColumnConfig,
  FiltersState,
} from "@/components/data-table-filter/core/types";
import {
  dbTypeToFilterType,
  getFilterTypeIcon,
  type FilterParam,
} from "@/lib/filter-utils";

const PAGE_SIZE = 100;

/**
 * Main DataViewer - splits by selectedTable condition
 */
export function DataViewer() {
  const { selectedTable } = useConnection();

  if (!selectedTable) {
    return <NoTableSelected />;
  }

  return <TableDataViewer selectedTable={selectedTable} />;
}

/**
 * Empty state when no table is selected
 */
function NoTableSelected() {
  const { connKey, connectionConfig } = useConnection();

  return (
    <div className="flex items-center justify-center h-full overflow-hidden">
      <div className="text-center space-y-3">
        <div className="space-y-1">
          <p className="text-sm text-muted-foreground">
            Connected to{" "}
            <span className="font-medium text-foreground">
              {connectionConfig.label || connKey}
            </span>
          </p>
        </div>
        <p className="text-xs text-muted-foreground">
          Select a table from the sidebar to view data
        </p>
      </div>
    </div>
  );
}

/**
 * Table data viewer - handles data fetching and display
 */
function TableDataViewer({ selectedTable }: { selectedTable: string }) {
  const { connKey, connectionConfig, projectPath, selectedSchema } =
    useConnection();

  const [page, setPage] = useState(0);
  const [sorting, setSorting] = useState<SortingState>([]);
  const [filters, setFilters] = useState<FiltersState>([]);
  // UI-only view toggle: the browse grid or the schema inspector. Reset
  // when the user navigates to a different table/collection so people
  // don't land on Structure for a row-oriented workflow.
  const [view, setView] = useState<"data" | "structure">("data");

  useEffect(() => {
    setPage(0);
    setSorting([]);
    setFilters([]);
    setView("data");
  }, [selectedTable, selectedSchema]);

  // Reset page when filters change
  useEffect(() => {
    setPage(0);
  }, [filters]);

  const objectKey = `${selectedSchema || ""}.${selectedTable}`;
  const engine = connectionConfig.engine;

  // Convert filters to backend format
  const filterParams: FilterParam[] = useMemo(
    () =>
      filters.map((f) => ({
        columnId: f.columnId,
        type: f.type,
        operator: f.operator,
        values: f.values as (string | number | boolean | null)[],
      })),
    [filters],
  );

  const dataQuery = useQuery({
    queryKey: [
      "table-data",
      projectPath,
      connKey,
      objectKey,
      page,
      sorting,
      filterParams,
    ],
    queryFn: async () => {
      const options: BrowseOptions = {
        limit: PAGE_SIZE,
        offset: page * PAGE_SIZE,
        orderByColumn: sorting[0]?.id ?? null,
        orderByDirection: sorting[0]
          ? sorting[0].desc
            ? "desc"
            : "asc"
          : null,
        filters: filterParams.length > 0 ? JSON.stringify(filterParams) : null,
      };

      switch (engine) {
        case "sqlite":
          return await cmd.querySqliteTable(
            projectPath,
            connKey,
            selectedTable,
            options,
          );

        case "postgres":
          return await cmd.queryPostgresTable(
            projectPath,
            connKey,
            selectedSchema || "public",
            selectedTable,
            options,
          );

        case "mongodb":
          return await cmd.queryMongodbCollection(
            projectPath,
            connKey,
            selectedTable,
            options,
          );

        default:
          throw new Error(`Unsupported engine: ${engine}`);
      }
    },
  });

  // We piggy-back on the existing describe_* command so the editor
  // and context-menu can know which columns are primary keys. This is
  // deliberately a separate query from the browse data so it stays
  // cached across pagination.
  const descriptionQuery = useQuery({
    queryKey: [
      "describe",
      projectPath,
      connKey,
      engine,
      selectedSchema || null,
      selectedTable,
    ],
    queryFn: async (): Promise<TableDescription> => {
      switch (engine) {
        case "sqlite":
          return await cmd.describeSqliteTable(
            projectPath,
            connKey,
            selectedTable,
          );
        case "postgres":
          return await cmd.describePostgresTable(
            projectPath,
            connKey,
            selectedSchema || "public",
            selectedTable,
          );
        case "mongodb":
          return await cmd.describeMongodbCollection(
            projectPath,
            connKey,
            selectedTable,
          );
        default:
          throw new Error(`Unsupported engine: ${engine}`);
      }
    },
    staleTime: 30_000,
  });

  const mutations = useRowMutations(selectedTable);
  const undoStack = useStore($undoStack);
  const canUndo = undoStack.length > 0;

  const [editorMode, setEditorMode] = useState<EditorMode | null>(null);

  const description = descriptionQuery.data;

  // Helper: build a pk payload from a row using the table's description.
  const buildPkForRow = useCallback(
    (row: RowMap): RowMap | null => {
      if (!description) return null;
      const pkCols = description.columns.filter((c) => c.isPrimaryKey);
      // For Mongo collections we fall back to `_id` even if the
      // sampler didn't flag it as PK — every document has one.
      if (pkCols.length === 0 && engine === "mongodb" && "_id" in row) {
        return { _id: row._id };
      }
      if (pkCols.length === 0) return null;
      const pk: RowMap = {};
      for (const c of pkCols) {
        pk[c.name] = row[c.name] ?? null;
      }
      return pk;
    },
    [description, engine],
  );

  const handleUndo = useCallback(async () => {
    try {
      await mutations.undo();
      toast.success("Undone");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : `Undo failed: ${String(e)}`);
    }
  }, [mutations]);

  const handleDelete = useCallback(
    async (row: RowMap) => {
      const pk = buildPkForRow(row);
      if (!pk) {
        toast.error("Cannot delete: this table has no primary key");
        return;
      }
      try {
        await mutations.deleteRow({ pk, originalRow: row });
        toast.success("Row deleted");
      } catch (e) {
        toast.error(
          e instanceof Error ? e.message : `Delete failed: ${String(e)}`,
        );
      }
    },
    [buildPkForRow, mutations],
  );

  // Keyboard undo: Cmd/Ctrl+Z. Scoped to the main window only so
  // detached child windows don't fire twice once pop-out lands.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const meta = e.metaKey || e.ctrlKey;
      if (meta && e.key.toLowerCase() === "z" && !e.shiftKey) {
        if ($undoStack.get().length === 0) return;
        e.preventDefault();
        handleUndo();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [handleUndo]);

  // Build filter column configs from query result
  const filterColumnConfigs: ColumnConfig<Record<string, unknown>>[] =
    useMemo(() => {
      if (dataQuery.status !== "success") return [];
      return dataQuery.data.columns.map((col) => {
        const filterType = dbTypeToFilterType(col.data_type);
        const Icon = getFilterTypeIcon(filterType);
        return {
          id: col.name,
          type: filterType,
          displayName: col.name,
          icon: Icon,
          accessor: (row: Record<string, unknown>) =>
            row[col.name] as string | number | Date,
        } as ColumnConfig<Record<string, unknown>>;
      });
    }, [dataQuery.status, dataQuery.data]);

  // Initialize filter hook with server strategy
  const filterInstance = useDataTableFilters({
    strategy: "server",
    data: [],
    columnsConfig: filterColumnConfigs,
    filters,
    onFiltersChange: setFilters,
  });

  // Use status for type narrowing
  switch (dataQuery.status) {
    case "pending":
      return (
        <div className="flex items-center justify-center h-full">
          <div className="flex flex-col items-center gap-3">
            <Loader2Icon className="size-5 animate-spin text-muted-foreground" />
            <p className="text-xs text-muted-foreground">Loading...</p>
          </div>
        </div>
      );

    case "error":
      return (
        <div className="flex items-center justify-center h-full">
          <div className="flex flex-col items-center gap-3 max-w-sm">
            <h2 className="text-sm font-medium text-destructive">
              Failed to load data
            </h2>
            <p className="text-xs text-muted-foreground text-center">
              {dataQuery.error instanceof Error
                ? dataQuery.error.message
                : "Unknown error"}
            </p>
            <Button
              variant="outline"
              size="sm"
              className="h-7 text-xs"
              onClick={() => dataQuery.refetch()}
            >
              <RefreshCwIcon className="size-3 mr-1.5" />
              Retry
            </Button>
          </div>
        </div>
      );

    case "success":
      break; // Continue to render
  }

  // At this point, dataQuery.data is guaranteed to exist
  const result = dataQuery.data;

  // Build columns for the data table
  const columns: ColumnDef<Record<string, unknown>>[] = result.columns.map(
    (col) => ({
      accessorKey: col.name,
      header: () => (
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="font-medium cursor-default">{col.name}</span>
          </TooltipTrigger>
          <TooltipContent side="bottom" className="text-xs">
            <span className="text-muted-foreground">Type:</span> {col.data_type}
          </TooltipContent>
        </Tooltip>
      ),
      cell: ({ getValue }) => {
        const value = getValue();
        return <CellValue value={value} />;
      },
    }),
  );

  // Convert rows to objects
  const data: Record<string, unknown>[] = result.rows.map((row) => {
    const obj: Record<string, unknown> = {};
    result.columns.forEach((col, idx) => {
      obj[col.name] = row[idx];
    });
    return obj;
  });

  const totalCount = result.total_count ?? 0;
  const totalPages = Math.ceil(totalCount / PAGE_SIZE);
  const startRow = page * PAGE_SIZE + 1;
  const endRow = Math.min((page + 1) * PAGE_SIZE, totalCount);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-1.5 border-b bg-muted/20">
        <div className="flex items-center gap-1.5">
          <TableIcon className="size-3.5 text-muted-foreground" />
          <h2 className="text-sm font-medium">
            {selectedSchema ? `${selectedSchema}.` : ""}
            {selectedTable}
          </h2>
        </div>
        <div className="flex items-center gap-1">
          <ViewToggle view={view} onChange={setView} />
          <span className="text-xs text-muted-foreground tabular-nums ml-2">
            {totalCount.toLocaleString()} rows
          </span>
          <Button
            variant="ghost"
            size="sm"
            className="h-6 px-2 text-[11px]"
            onClick={() => setEditorMode({ kind: "insert" })}
            disabled={!description}
            title="Insert new row"
          >
            <PlusIcon className="size-3 mr-1" />
            New
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="size-6"
            onClick={handleUndo}
            disabled={!canUndo}
            title={
              canUndo
                ? `Undo: ${undoStack[undoStack.length - 1]?.label}`
                : "Nothing to undo"
            }
          >
            <UndoIcon className="size-3.5" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="size-6"
            onClick={() => dataQuery.refetch()}
            disabled={dataQuery.isFetching}
          >
            <RefreshCwIcon
              className={`size-3.5 ${dataQuery.isFetching ? "animate-spin" : ""}`}
            />
          </Button>
        </div>
      </div>

      {view === "structure" ? (
        <div className="flex-1 min-h-0">
          <SchemaInspector selectedTable={selectedTable} />
        </div>
      ) : (
        <>
          {filterColumnConfigs.length > 0 && (
            <div className="px-3 py-1.5 border-b">
              <DataTableFilter
                columns={filterInstance.columns}
                filters={filterInstance.filters}
                actions={filterInstance.actions}
                strategy={filterInstance.strategy}
              />
            </div>
          )}

          <div className="flex-1 min-h-0">
            <DataTable
              columns={columns}
              data={data}
              sorting={sorting}
              onSortingChange={setSorting}
              renderRowContextMenu={(row) => {
                const pk = buildPkForRow(row);
                const canMutate = !!pk;
                return (
                  <ContextMenuContent className="text-xs">
                    <ContextMenuItem
                      className="text-xs"
                      disabled={!canMutate}
                      onClick={() => {
                        if (!pk) return;
                        setEditorMode({
                          kind: "edit",
                          pk,
                          originalRow: row,
                        });
                      }}
                    >
                      <PencilIcon className="size-3 mr-2" />
                      Edit row...
                    </ContextMenuItem>
                    <ContextMenuItem
                      className="text-xs"
                      disabled={!description}
                      onClick={() => {
                        // Duplicate: open insert editor pre-populated
                        // with the row's values minus its PKs (so the
                        // database can assign fresh ones).
                        if (!description) return;
                        const clone: RowMap = { ...row };
                        for (const c of description.columns) {
                          if (c.isPrimaryKey) delete clone[c.name];
                        }
                        // Stash into editor state by temporarily
                        // overriding "insert" mode with a seeded row.
                        // We reuse edit mode internally by using a
                        // synthetic originalRow path — simpler to
                        // open a fresh insert and let the user paste.
                        setEditorMode({ kind: "insert" });
                        toast.message(
                          "Duplicate: open insert with this row's non-PK fields (TODO)",
                        );
                        void clone;
                      }}
                    >
                      <CopyIcon className="size-3 mr-2" />
                      Duplicate
                    </ContextMenuItem>
                    <ContextMenuSeparator />
                    <ContextMenuItem
                      className="text-xs text-destructive focus:text-destructive"
                      disabled={!canMutate}
                      onClick={() => handleDelete(row)}
                    >
                      <TrashIcon className="size-3 mr-2" />
                      Delete row
                    </ContextMenuItem>
                  </ContextMenuContent>
                );
              }}
            />
          </div>

          {/* Footer pagination */}
          {totalPages > 1 && (
            <div className="flex items-center justify-between px-3 py-1.5 border-t bg-muted/20 text-xs">
              <span className="text-muted-foreground tabular-nums">
                {startRow.toLocaleString()}–{endRow.toLocaleString()} of{" "}
                {totalCount.toLocaleString()}
              </span>
              <div className="flex items-center gap-0.5">
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-6"
                  onClick={() => setPage(0)}
                  disabled={page === 0}
                >
                  <ChevronsLeftIcon className="size-3.5" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-6"
                  onClick={() => setPage((p) => Math.max(0, p - 1))}
                  disabled={page === 0}
                >
                  <ChevronLeftIcon className="size-3.5" />
                </Button>
                <span className="px-2 text-muted-foreground tabular-nums min-w-[60px] text-center">
                  {page + 1} / {totalPages}
                </span>
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-6"
                  onClick={() =>
                    setPage((p) => Math.min(totalPages - 1, p + 1))
                  }
                  disabled={page >= totalPages - 1}
                >
                  <ChevronRightIcon className="size-3.5" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-6"
                  onClick={() => setPage(totalPages - 1)}
                  disabled={page >= totalPages - 1}
                >
                  <ChevronsRightIcon className="size-3.5" />
                </Button>
              </div>
            </div>
          )}
        </>
      )}

      {description && editorMode ? (
        <RowEditorDialog
          open={!!editorMode}
          onOpenChange={(o) => {
            if (!o) setEditorMode(null);
          }}
          selectedTable={selectedTable}
          description={description}
          mode={editorMode}
        />
      ) : null}
    </div>
  );
}

/**
 * Segmented toggle between the data grid and the schema inspector.
 * Mirrors the Data/Structure tabs you'd expect from DataGrip/TablePlus
 * but kept inside the viewer so we don't fight the outer workspace
 * router yet (see Phase 2 tabs todo for the real tabbed workspace).
 */
function ViewToggle({
  view,
  onChange,
}: {
  view: "data" | "structure";
  onChange: (v: "data" | "structure") => void;
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

// Helper component to render cell values
function CellValue({ value }: { value: unknown }) {
  if (value === null || value === undefined) {
    return <span className="text-muted-foreground/60 italic">null</span>;
  }

  if (typeof value === "boolean") {
    return (
      <span
        className={
          value
            ? "text-emerald-600 dark:text-emerald-400"
            : "text-red-500 dark:text-red-400"
        }
      >
        {value.toString()}
      </span>
    );
  }

  if (typeof value === "number") {
    return (
      <span className="text-blue-600 dark:text-blue-400">
        {value.toLocaleString()}
      </span>
    );
  }

  if (typeof value === "object") {
    const json = JSON.stringify(value);
    return (
      <span
        className="text-amber-600 dark:text-amber-400 max-w-[200px] truncate inline-block align-bottom"
        title={json}
      >
        {json}
      </span>
    );
  }

  const strValue = String(value);
  if (strValue.length > 80) {
    return (
      <span
        className="max-w-[300px] truncate inline-block align-bottom"
        title={strValue}
      >
        {strValue}
      </span>
    );
  }

  return <span>{strValue}</span>;
}

/**
 * DataViewer — browses a single table or collection.
 *
 * After the Phase 2 decomposition this file owns:
 *  - the browse / describe queries and their invalidation
 *  - the editing state (editor dialog + undo stack + pop-out)
 *  - composition of the purpose-built sub-components
 *    (`DataViewerHeader`, `ViewToggle`, `TablePaginationFooter`, …)
 *
 * Everything visually-cohesive (header layout, pagination strip,
 * schema inspector, row editor dialog, cell rendering) lives in its
 * own file. Adding a new DataViewer action is now a one-prop change
 * on `DataViewerHeader`, not a scroll through a 700-LOC file.
 */

import { useStore } from "@nanostores/react";
import { useQuery } from "@tanstack/react-query";
import type { ColumnDef, SortingState } from "@tanstack/react-table";
import {
  CopyIcon,
  Loader2Icon,
  PencilIcon,
  RefreshCwIcon,
  TrashIcon,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { type BrowseOptions, cmd } from "@/commands";
import { DataTableFilter } from "@/components/data-table-filter/components/data-table-filter";
import type {
  ColumnConfig,
  FiltersState,
} from "@/components/data-table-filter/core/types";
import { useDataTableFilters } from "@/components/data-table-filter/hooks/use-data-table-filters";
import { Button } from "@/components/ui/button";
import {
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
} from "@/components/ui/context-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { VirtualDataTable } from "@/components/virtual-data-table";
import { CellDetailPanel } from "@/components/workspace/cell-detail-panel";
import { CellValue } from "@/components/workspace/cell-value";
import { DataViewerHeader } from "@/components/workspace/data-viewer-header";
import {
  type EditorMode,
  RowEditorDialog,
} from "@/components/workspace/row-editor-dialog";
import { SchemaInspector } from "@/components/workspace/schema-inspector";
import { TablePaginationFooter } from "@/components/workspace/table-pagination-footer";
import type { TableView } from "@/components/workspace/view-toggle";
import { type RowMap, useRowMutations } from "@/hooks/use-row-mutations";
import { useWindow } from "@/hooks/use-window";
import { useWorkspace } from "@/hooks/use-workspace";
import { exportAsCsv, exportAsJson } from "@/lib/export";
import {
  dbTypeToFilterType,
  type FilterParam,
  getFilterTypeIcon,
} from "@/lib/filter-utils";
import { queryKeys } from "@/lib/query-keys";
import { $undoStack } from "@/stores/row-mutations-store";
import type { TableDescription } from "@/types/project";

const PAGE_SIZE = 100;

/**
 * Shell component — branches on whether a table is selected. Keeps
 * the hook order stable and lets the two states be laid out
 * independently.
 */
export function DataViewer() {
  const { selectedTable } = useWorkspace();
  if (!selectedTable) {
    return <NoTableSelected />;
  }
  return <TableDataViewer selectedTable={selectedTable} />;
}

function NoTableSelected() {
  const { connKey, connectionConfig } = useWorkspace();
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

function TableDataViewer({ selectedTable }: { selectedTable: string }) {
  const { connKey, connectionConfig, projectPath, selectedSchema, engine } =
    useWorkspace();

  const [page, setPage] = useState(0);
  const [sorting, setSorting] = useState<SortingState>([]);
  const [filters, setFilters] = useState<FiltersState>([]);
  // UI-only view toggle: the browse grid or the schema inspector. Reset
  // when the user navigates to a different table/collection so people
  // don't land on Structure for a row-oriented workflow.
  const [view, setView] = useState<TableView>("data");

  useEffect(() => {
    setPage(0);
    setSorting([]);
    setFilters([]);
    setView("data");
  }, [selectedTable, selectedSchema]);

  useEffect(() => {
    setPage(0);
  }, [filters]);

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
    queryKey: queryKeys.conn.tableData(projectPath, connKey, {
      schema: selectedSchema ?? null,
      name: selectedTable,
      page,
      sorting,
      filters: filterParams,
    }),
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
    queryKey: queryKeys.conn.tableDescribe(
      projectPath,
      connKey,
      engine,
      selectedSchema,
      selectedTable,
    ),
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

  const { isMain, openTab } = useWindow();

  const handlePopOut = useCallback(async () => {
    if (!isMain) return;
    try {
      await openTab({
        kind: "table",
        connection: { project: projectPath, conn_key: connKey },
        schema: selectedSchema ?? null,
        name: selectedTable,
      });
    } catch (e) {
      toast.error(
        e instanceof Error ? `Couldn't open window: ${e.message}` : String(e),
      );
    }
  }, [isMain, openTab, projectPath, connKey, selectedSchema, selectedTable]);

  const [editorMode, setEditorMode] = useState<EditorMode | null>(null);
  const [detailCell, setDetailCell] = useState<{
    columnId: string;
    value: unknown;
  } | null>(null);

  const description = descriptionQuery.data;

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
  // detached child windows don't fire twice.
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

  const filterInstance = useDataTableFilters({
    strategy: "server",
    data: [],
    columnsConfig: filterColumnConfigs,
    filters,
    onFiltersChange: setFilters,
  });

  if (dataQuery.status === "pending") {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="flex flex-col items-center gap-3">
          <Loader2Icon className="size-5 animate-spin text-muted-foreground" />
          <p className="text-xs text-muted-foreground">Loading...</p>
        </div>
      </div>
    );
  }

  if (dataQuery.status === "error") {
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
  }

  const result = dataQuery.data;

  const columns: ColumnDef<Record<string, unknown>>[] = result.columns.map(
    (col) => ({
      accessorKey: col.name,
      header: () => (
        <Tooltip>
          <TooltipTrigger
            render={
              <span className="font-medium cursor-default">{col.name}</span>
            }
          />
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

  const data: Record<string, unknown>[] = result.rows.map((row) => {
    const obj: Record<string, unknown> = {};
    result.columns.forEach((col, idx) => {
      obj[col.name] = row[idx];
    });
    return obj;
  });

  const totalCount = result.total_count ?? 0;
  const totalPages = Math.ceil(totalCount / PAGE_SIZE);

  // `connectionConfig` is kept available so future header features
  // (engine badge, connection label) don't need another hook call.
  void connectionConfig;

  return (
    <div className="flex flex-col h-full">
      <DataViewerHeader
        selectedTable={selectedTable}
        selectedSchema={selectedSchema}
        totalCount={totalCount}
        view={view}
        onViewChange={setView}
        onNewRow={() => setEditorMode({ kind: "insert" })}
        canInsert={!!description}
        onExportCsv={() =>
          exportAsCsv(
            selectedTable,
            result.columns.map((c) => c.name),
            data,
          )
        }
        onExportJson={() =>
          exportAsJson(
            selectedTable,
            result.columns.map((c) => c.name),
            data,
          )
        }
        canExport={data.length > 0}
        onUndo={handleUndo}
        canUndo={canUndo}
        undoLabel={undoStack[undoStack.length - 1]?.label}
        onPopOut={isMain ? handlePopOut : null}
        onRefresh={() => dataQuery.refetch()}
        isRefreshing={dataQuery.isFetching}
      />

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

          <div className="flex-1 min-h-0 flex flex-col">
            <div className="flex-1 min-h-0">
              <VirtualDataTable
                columns={columns}
                data={data}
                sorting={sorting}
                onSortingChange={setSorting}
                onCellClick={({ columnId, value }) =>
                  setDetailCell({ columnId, value })
                }
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
            {detailCell ? (
              <div className="h-[30%] min-h-[120px] max-h-[50%] shrink-0">
                <CellDetailPanel
                  columnId={detailCell.columnId}
                  value={detailCell.value}
                  onClose={() => setDetailCell(null)}
                />
              </div>
            ) : null}
          </div>

          <TablePaginationFooter
            page={page}
            totalPages={totalPages}
            totalCount={totalCount}
            pageSize={PAGE_SIZE}
            onPageChange={setPage}
          />
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

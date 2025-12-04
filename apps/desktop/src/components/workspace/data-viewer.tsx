import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import {
  Loader2Icon,
  RefreshCwIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  ChevronsLeftIcon,
  ChevronsRightIcon,
  TableIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { DataTable } from "@/components/data-table";
import { useConnection } from "@/routes/project.$projectId/conn.$connKey";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { ColumnDef } from "@tanstack/react-table";

interface QueryResult {
  columns: { name: string; data_type: string }[];
  rows: unknown[][];
  total_count: number | null;
}

const PAGE_SIZE = 100;

export function DataViewer() {
  const { connKey, connectionConfig, projectPath, selectedTable, selectedSchema } = useConnection();

  const [page, setPage] = useState(0);

  // Reset page when object changes
  const objectKey = selectedTable
    ? `${selectedSchema || ""}.${selectedTable}`
    : null;

  const engine = connectionConfig.engine;

  const dataQuery = useQuery({
    queryKey: [
      "table-data",
      projectPath,
      connKey,
      objectKey,
      page,
    ],
    queryFn: async () => {
      if (!selectedTable) {
        throw new Error("No table selected");
      }

      const offset = page * PAGE_SIZE;

      switch (engine) {
        case "sqlite":
          return await invoke<QueryResult>("query_sqlite_table", {
            projectPath,
            connKey,
            tableName: selectedTable,
            limit: PAGE_SIZE,
            offset,
          });

        case "postgres":
          return await invoke<QueryResult>("query_postgres_table", {
            projectPath,
            connKey,
            schema: selectedSchema || "public",
            tableName: selectedTable,
            limit: PAGE_SIZE,
            offset,
          });

        case "mongodb":
          return await invoke<QueryResult>("query_mongodb_collection", {
            projectPath,
            connKey,
            collectionName: selectedTable,
            limit: PAGE_SIZE,
            offset,
          });

        default:
          throw new Error(`Unsupported engine: ${engine}`);
      }
    },
    enabled: !!selectedTable,
  });

  // Show empty state when connected but no table selected
  if (!selectedTable) {
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

  if (dataQuery.isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="flex flex-col items-center gap-3">
          <Loader2Icon className="size-5 animate-spin text-muted-foreground" />
          <p className="text-xs text-muted-foreground">Loading...</p>
        </div>
      </div>
    );
  }

  if (dataQuery.isError) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="flex flex-col items-center gap-3 max-w-sm">
          <h2 className="text-sm font-medium text-destructive">Failed to load data</h2>
          <p className="text-xs text-muted-foreground text-center">
            {dataQuery.error instanceof Error ? dataQuery.error.message : "Unknown error"}
          </p>
          <Button variant="outline" size="sm" className="h-7 text-xs" onClick={() => dataQuery.refetch()}>
            <RefreshCwIcon className="size-3 mr-1.5" />
            Retry
          </Button>
        </div>
      </div>
    );
  }

  const result = dataQuery.data;
  if (!result) return null;

  // Build columns for the data table
  const columns: ColumnDef<Record<string, unknown>>[] = result.columns.map((col) => ({
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
  }));

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
          <span className="text-xs text-muted-foreground tabular-nums">
            {totalCount.toLocaleString()} rows
          </span>
          <Button
            variant="ghost"
            size="icon"
            className="size-6"
            onClick={() => dataQuery.refetch()}
            disabled={dataQuery.isFetching}
          >
            <RefreshCwIcon className={`size-3.5 ${dataQuery.isFetching ? "animate-spin" : ""}`} />
          </Button>
        </div>
      </div>

      {/* Data Table */}
      <div className="flex-1 overflow-auto">
        <DataTable columns={columns} data={data} />
      </div>

      {/* Footer pagination */}
      {totalPages > 1 && (
        <div className="flex items-center justify-between px-3 py-1.5 border-t bg-muted/20 text-xs">
          <span className="text-muted-foreground tabular-nums">
            {startRow.toLocaleString()}–{endRow.toLocaleString()} of {totalCount.toLocaleString()}
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
              onClick={() => setPage((p) => Math.min(totalPages - 1, p + 1))}
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
      <span className={value ? "text-emerald-600 dark:text-emerald-400" : "text-red-500 dark:text-red-400"}>
        {value.toString()}
      </span>
    );
  }

  if (typeof value === "number") {
    return <span className="text-blue-600 dark:text-blue-400">{value.toLocaleString()}</span>;
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
      <span className="max-w-[300px] truncate inline-block align-bottom" title={strValue}>
        {strValue}
      </span>
    );
  }

  return <span>{strValue}</span>;
}

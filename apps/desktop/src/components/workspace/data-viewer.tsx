import { useStore } from "@nanostores/react";
import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { Loader2Icon, RefreshCwIcon, ChevronLeftIcon, ChevronRightIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { DataTable } from "@/components/data-table";
import {
  $selectedObject,
  $activeConnection,
  $projectPath,
  $projectConfig,
} from "@/stores/project-state";
import type { ColumnDef } from "@tanstack/react-table";

interface QueryResult {
  columns: { name: string; data_type: string }[];
  rows: unknown[][];
  total_count: number | null;
}

const PAGE_SIZE = 100;

export function DataViewer() {
  const selectedObject = useStore($selectedObject);
  const activeConnection = useStore($activeConnection);
  const projectPath = useStore($projectPath);
  const projectConfig = useStore($projectConfig);

  const [page, setPage] = useState(0);

  // Reset page when object changes
  const objectKey = selectedObject
    ? `${selectedObject.schema || ""}.${selectedObject.name}`
    : null;

  const engine = activeConnection && projectConfig
    ? projectConfig.connection[activeConnection]?.engine
    : null;

  const dataQuery = useQuery({
    queryKey: [
      "table-data",
      projectPath,
      activeConnection,
      objectKey,
      page,
    ],
    queryFn: async () => {
      if (!projectPath || !activeConnection || !selectedObject) {
        throw new Error("Missing required parameters");
      }

      const offset = page * PAGE_SIZE;

      switch (engine) {
        case "sqlite":
          return await invoke<QueryResult>("query_sqlite_table", {
            projectPath,
            connKey: activeConnection,
            tableName: selectedObject.name,
            limit: PAGE_SIZE,
            offset,
          });

        case "postgres":
          return await invoke<QueryResult>("query_postgres_table", {
            projectPath,
            connKey: activeConnection,
            schema: selectedObject.schema || "public",
            tableName: selectedObject.name,
            limit: PAGE_SIZE,
            offset,
          });

        case "mongodb":
          return await invoke<QueryResult>("query_mongodb_collection", {
            projectPath,
            connKey: activeConnection,
            collectionName: selectedObject.name,
            limit: PAGE_SIZE,
            offset,
          });

        default:
          throw new Error(`Unsupported engine: ${engine}`);
      }
    },
    enabled: !!projectPath && !!activeConnection && !!selectedObject && !!engine,
  });

  if (!selectedObject) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center space-y-4">
          <h2 className="text-2xl font-bold">Welcome to Your Project</h2>
          <p className="text-muted-foreground">
            Select a table or collection from the sidebar to explore
          </p>
        </div>
      </div>
    );
  }

  if (dataQuery.isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="flex flex-col items-center gap-4">
          <Loader2Icon className="size-8 animate-spin text-muted-foreground" />
          <p className="text-sm text-muted-foreground">Loading data...</p>
        </div>
      </div>
    );
  }

  if (dataQuery.isError) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="flex flex-col items-center gap-4 max-w-md">
          <h2 className="text-lg font-semibold text-destructive">Failed to load data</h2>
          <p className="text-sm text-muted-foreground text-center">
            {dataQuery.error instanceof Error ? dataQuery.error.message : "Unknown error"}
          </p>
          <Button variant="outline" onClick={() => dataQuery.refetch()}>
            <RefreshCwIcon className="size-4 mr-2" />
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
      <div className="flex flex-col">
        <span className="font-semibold">{col.name}</span>
        <span className="text-xs text-muted-foreground font-normal">{col.data_type}</span>
      </div>
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
      <div className="flex items-center justify-between px-4 py-2 border-b bg-muted/30">
        <div className="flex items-center gap-2">
          <h2 className="font-semibold">
            {selectedObject.schema ? `${selectedObject.schema}.` : ""}
            {selectedObject.name}
          </h2>
          <span className="text-xs text-muted-foreground">
            ({selectedObject.type})
          </span>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-sm text-muted-foreground">
            {totalCount.toLocaleString()} rows
          </span>
          <Button
            variant="ghost"
            size="icon"
            onClick={() => dataQuery.refetch()}
            disabled={dataQuery.isFetching}
          >
            <RefreshCwIcon className={`size-4 ${dataQuery.isFetching ? "animate-spin" : ""}`} />
          </Button>
        </div>
      </div>

      {/* Data Table */}
      <div className="flex-1 overflow-auto p-2">
        <DataTable
          columns={columns}
          data={data}
          extraFooter={
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <span>
                Showing {startRow.toLocaleString()} - {endRow.toLocaleString()} of{" "}
                {totalCount.toLocaleString()}
              </span>
              <div className="flex items-center gap-1">
                <Button
                  variant="outline"
                  size="icon"
                  className="h-7 w-7"
                  onClick={() => setPage((p) => Math.max(0, p - 1))}
                  disabled={page === 0}
                >
                  <ChevronLeftIcon className="size-4" />
                </Button>
                <span className="px-2">
                  Page {page + 1} of {totalPages || 1}
                </span>
                <Button
                  variant="outline"
                  size="icon"
                  className="h-7 w-7"
                  onClick={() => setPage((p) => Math.min(totalPages - 1, p + 1))}
                  disabled={page >= totalPages - 1}
                >
                  <ChevronRightIcon className="size-4" />
                </Button>
              </div>
            </div>
          }
        />
      </div>
    </div>
  );
}

// Helper component to render cell values
function CellValue({ value }: { value: unknown }) {
  if (value === null || value === undefined) {
    return <span className="text-muted-foreground italic">NULL</span>;
  }

  if (typeof value === "boolean") {
    return <span className={value ? "text-green-600" : "text-red-600"}>{value.toString()}</span>;
  }

  if (typeof value === "number") {
    return <span>{value.toLocaleString()}</span>;
  }

  if (typeof value === "object") {
    return (
      <span className="text-xs bg-muted px-1 py-0.5 rounded font-mono">
        {JSON.stringify(value)}
      </span>
    );
  }

  const strValue = String(value);
  // Truncate long strings
  if (strValue.length > 100) {
    return (
      <span title={strValue}>
        {strValue.slice(0, 100)}...
      </span>
    );
  }

  return <span>{strValue}</span>;
}

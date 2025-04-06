import { query } from "@/commands.ts";
import { DataTable } from "@/components/data-table.tsx";
import type { DbConnectionMeta } from "@/stores";
import { buildConnString } from "@/utils";
import { useQuery } from "@tanstack/react-query";
import { KeyRoundIcon, RefreshCcwIcon, Rows3Icon, TimerIcon } from "lucide-react";
import { useMemo } from "react";
import { useConnection } from "@/queries/use-connection";
import { Link } from "@tanstack/react-router";
import { Button } from "@/components/ui/button";

type ColumnInfo = {
  index: number;
  name: string;
  type: string;
  pk: boolean;
};

export function TableView({
  connection: connMeta,
  tableName,
}: {
  connection: DbConnectionMeta;
  tableName: string;
}) {
  const connString = buildConnString(connMeta);

  // Use the connection hook
  const { status: connectionStatus, retry } = useConnection(connMeta.id);

  const tableQuery = useQuery({
    queryKey: ["connection", connMeta.id, "table", tableName],
    queryFn: async () => {
      const queryTime = performance.now();
      const { result: tableInfo } = await query<Array<Record<string, any>>>(
        connString,
        `PRAGMA table_info("${tableName}")`,
        [],
      );

      const columns = tableInfo.map(
        (column) =>
          ({
            index: column.cid,
            name: column.name,
            type: column.type,
            pk: column.pk === 1,
          }) as ColumnInfo,
      );

      const { result } = await query<Array<Record<string, any>>>(
        connString,
        `SELECT rowid, * FROM "${tableName}"`,
        [],
      );
      const endQueryTime = performance.now();
      return {
        columns: [
          { index: -1, name: "rowid", type: "INTEGER", pk: false },
          ...columns,
        ],
        results: result,
        queryTime: endQueryTime - queryTime,
      };
    },
    enabled: connectionStatus.status === 'success', // Only run when connection is successful
  });

  // Using exhaustive switch pattern for better type checking
  switch (connectionStatus.status) {
    case 'error':
      return (
        <div className="flex flex-col items-center justify-center h-full gap-4 p-4">
          <div className="text-destructive text-lg font-medium">Connection Error</div>
          <div className="text-destructive/80 text-center max-w-md">
            {connectionStatus.error.message}
          </div>
          <div className="flex gap-4 mt-4">
            <Button
              variant="outline"
              onClick={retry}
              className="flex items-center gap-2"
            >
              <RefreshCcwIcon className="size-4" />
              Retry Connection
            </Button>
            <Button asChild>
              <Link to="/">
                Go Home
              </Link>
            </Button>
          </div>
        </div>
      );
    case 'loading':
      return <div className="p-4">Connecting to database...</div>;
    case 'success':
      if (tableQuery.isPending) {
        return <div className="p-4">Loading table data...</div>;
      }

      if (tableQuery.isError) {
        return <div className="p-4 text-destructive">Error: {tableQuery.error.message}</div>;
      }

      return (
        <TableViewMain
          columns={tableQuery.data.columns}
          results={tableQuery.data.results}
          queryTime={tableQuery.data.queryTime}
        />
      );
  }
}

export function TableViewMain({
  columns,
  results,
  queryTime,
}: {
  columns: ColumnInfo[];
  results: object[];
  queryTime: number;
}) {
  const columnDefs = useMemo(() => {
    return columns.map((column) => ({
      accessorKey: column.name,
      header: () => (
        <div className="flex items-start gap-1 min-w-32">
          {column.pk && <KeyRoundIcon className="size-4 mt-1 text-yellow-500" />}
          <div className="flex flex-col font-mono">
            {column.name}
            <span className="text-xs font-light text-muted-foreground">{column.type}</span>
          </div>
        </div>
      ),
      cell: ({ row }: { row: any }) => {
        const cellValue = row.getValue(column.name);

        if (!cellValue) {
          return <span className="text-muted-foreground/50">NULL</span>;
        }

        if (column.type === "INTEGER") {
          return cellValue;
        }

        if (column.type === "BLOB") {
          return (
            <span className="text-muted-foreground">
              {cellValue.length} bytes
            </span>
          );
        }

        return cellValue;
      },
    }));
  }, [columns]);

  return (
    <DataTable
      columns={columnDefs}
      data={results}
      extraFooter={
        <>
          <div className="flex items-center gap-1 text-sm text-muted-foreground">
            <TimerIcon className="size-4" />
            <span>{queryTime.toFixed(0)}ms</span>
          </div>
          <div className="flex items-center gap-1 text-sm text-muted-foreground">
            <Rows3Icon className="size-4" />
            <span>{results.length} rows</span>
          </div>
        </>
      }
    />
  );
}

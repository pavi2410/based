import { query } from "@/commands.ts";
import { DataTable } from "@/components/data-table.tsx";
import type { DbConnectionMeta } from "@/stores.ts";
import { buildConnString } from "@/utils";
import { useQuery } from "@tanstack/react-query";
import { KeyRoundIcon, Rows3Icon, TimerIcon } from "lucide-react";
import { useMemo } from "react";

type ColumnInfo = {
  index: number;
  name: string;
  type: string;
  pk: boolean;
};

export function TableView({
  connection,
  tableName,
}: {
  connection: DbConnectionMeta;
  tableName: string;
}) {
  const tableQuery = useQuery({
    queryKey: ["conn", connection.id, "table", tableName],
    queryFn: async () => {
      const connString = buildConnString(connection);
      const queryTime = performance.now();
      const tableInfo = await query(
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

      const results = await query(
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
        results,
        queryTime: endQueryTime - queryTime,
      };
    },
  });

  if (tableQuery.status === "pending") {
    return <div className="p-2">Loading...</div>;
  }

  if (tableQuery.status === "error") {
    return <div className="p-2">Error: {tableQuery.error.toString()}</div>;
  }

  return (
    <TableViewMain
      columns={tableQuery.data.columns}
      results={tableQuery.data.results}
      queryTime={tableQuery.data.queryTime}
    />
  );
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
          {column.pk && <KeyRoundIcon className="size-4 mt-1" />}
          <div className="flex flex-col font-mono">
            {column.name}
            <span className="text-xs font-light">{column.type}</span>
          </div>
        </div>
      ),
      cell: ({ row }) => {
        if (column.type === "INTEGER") {
          return row.getValue(column.name);
        }

        if (column.type === "BLOB") {
          return (
            <span className="text-muted-foreground">
              {row.getValue(column.name).length} bytes
            </span>
          );
        }

        return row.getValue(column.name);
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
            <span>{queryTime.toFixed(2)}ms</span>
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

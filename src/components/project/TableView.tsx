import { query } from "@/commands.ts";
import { DataTable } from "@/components/data-table.tsx";
import type { DbConnectionMeta } from "@/stores.ts";
import { buildConnString } from "@/utils";
import { useQuery } from "@tanstack/react-query";
import { Rows3Icon, TimerIcon } from "lucide-react";
import { useMemo } from "react";

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
      const results = await query(connString, `SELECT * FROM ${tableName}`, []);
      const endQueryTime = performance.now();
      return {
        results,
        queryTime: endQueryTime - queryTime,
      };
    },
  });

  if (tableQuery.status === "pending") {
    return <div className="p-2">Loading...</div>;
  }

  if (tableQuery.status === "error") {
    return <div className="p-2">Error: {tableQuery.error.message}</div>;
  }

  return (
    <TableViewMain
      results={tableQuery.data.results}
      queryTime={tableQuery.data.queryTime}
    />
  );
}

export function TableViewMain({
  results,
  queryTime,
}: {
  results: object[];
  queryTime: number;
}) {
  const columns = useMemo(() => {
    if (results.length === 0) {
      return [];
    }

    return Object.keys(results[0]).map((key) => ({
      accessorKey: key,
      header: key,
    }));
  }, [results]);

  return (
    <DataTable
      columns={columns}
      data={results}
      extraFooter={
        <>
          <div className="flex items-center gap-1 text-sm text-muted-foreground">
            <TimerIcon className="size-5" />
            <span>{queryTime.toFixed(2)}ms</span>
          </div>
          <div className="flex items-center gap-1 text-sm text-muted-foreground">
            <Rows3Icon className="size-5" />
            <span>{results.length} rows</span>
          </div>
        </>
      }
    />
  );
}

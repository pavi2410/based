import {useQuery} from "@tanstack/react-query";
import {getConnection} from "@/stores.ts";
import {DataTable} from "@/components/data-table.tsx";
import {query} from "@/commands.ts";
import {useMemo} from "react";
import {Rows3Icon, TimerIcon} from "lucide-react";

export function TableView({projectId, connectionId, tableName}: {
  projectId: string,
  connectionId: string,
  tableName: string
}) {
  const connectionQuery = useQuery({
    queryKey: ['projects', projectId, 'connections', connectionId],
    queryFn: async () => {
      return await getConnection(projectId, connectionId)
    },
  })

  const tableQuery = useQuery({
    enabled: connectionQuery.status === 'success',
    queryKey: ['projects', projectId, 'connections', connectionId, 'table', tableName],
    queryFn: async () => {
      const connString = `sqlite:${connectionQuery.data!.filePath}`
      const queryTime = performance.now()
      const results = await query(connString, `SELECT *
                                               FROM ${tableName}`, [])
      const endQueryTime = performance.now()
      return {
        results,
        queryTime: endQueryTime - queryTime,
      }
    },
  })

  if (connectionQuery.status === 'pending' || tableQuery.status === 'pending') {
    return <div className="p-2">Loading...</div>
  }

  if (connectionQuery.status === 'error' || tableQuery.status === 'error') {
    return <div className="p-2">Error: {connectionQuery.error?.message ?? tableQuery.error?.message ?? ''}</div>
  }

  if (!connectionQuery.data) {
    return <div className="p-2">Connection not found</div>
  }

  return <TableViewMain results={tableQuery.data.results} queryTime={tableQuery.data.queryTime}/>
}

export function TableViewMain({results, queryTime}: {
  results: object[],
  queryTime: number,
}) {

  const columns = useMemo(() => {
    if (results.length === 0) {
      return []
    }

    return Object.keys(results[0]).map((key) => ({
      accessorKey: key,
      header: key,
    }))
  }, [results])

  return (
    <DataTable
      columns={columns}
      data={results}
      extraFooter={
        <>
          <div className="flex items-center gap-1 text-sm text-muted-foreground">
            <TimerIcon className="size-5"/>
            <span>{queryTime.toFixed(2)}ms</span>
          </div>
          <div className="flex items-center gap-1 text-sm text-muted-foreground">
            <Rows3Icon className="size-5"/>
            <span>{results.length} rows</span>
          </div>
        </>
      }
    />
  )
}
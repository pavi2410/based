import {useState} from "react";
import {useMutation, useQuery} from "@tanstack/react-query";
import {query} from "@/commands.ts";
import {toast} from "@/hooks/use-toast.ts";
import {DbConnectionMeta, getConnection} from "@/stores.ts";
import {Textarea} from "@/components/ui/textarea.tsx";
import {Button} from "@/components/ui/button.tsx";
import {Loader2Icon, Rows3Icon, TimerIcon} from "lucide-react";

export function QueryView({projectId, connectionId}: { projectId: string, connectionId: string }) {
  const connectionQuery = useQuery({
    queryKey: ['projects', projectId, 'connections', connectionId],
    queryFn: async () => {
      return await getConnection(projectId, connectionId)
    },
  })

  if (connectionQuery.status === 'pending') {
    return <div className="p-2">Loading...</div>
  }

  if (connectionQuery.status === 'error') {
    return <div className="p-2">Error: {connectionQuery.error.message}</div>
  }

  if (!connectionQuery.data) {
    return <div className="p-2">Connection not found</div>
  }

  return <QueryViewMain connection={connectionQuery.data}/>
}

function QueryViewMain({connection}: { connection: DbConnectionMeta }) {
  const [queryText, setQueryText] = useState('')

  const queryMutation = useMutation({
    mutationFn: async () => {
      const connString = `sqlite:${connection.filePath}`
      const queryTime = performance.now()
      const results = await query(connString, queryText, [])
      const endQueryTime = performance.now()
      return {
        results,
        queryTime: endQueryTime - queryTime,
      }
    },
    onSuccess: () => {
      toast({
        title: 'Executed',
      })
    },
    onError: (err) => {
      console.log('query', err)
    },
  })

  return (
    <div className="flex flex-col h-full">
      <div className="flex flex-col *:flex-1 flex-1 *:rounded-none">
        <Textarea
          value={queryText}
          onChange={(e) => setQueryText(e.target.value)}
        />

        <Textarea
          value={queryMutation.data != null ? JSON.stringify(queryMutation.data.results, null, 2) : queryMutation.isPending ? 'Running...' : ''}
          readOnly
        />
      </div>
      <div className="flex justify-end gap-4 p-2">
        {queryMutation.data != null && (
          <>
            <div className="flex items-center gap-1 text-sm text-muted-foreground">
              <TimerIcon className="size-5"/>
              <span>{queryMutation.data.queryTime.toFixed(2)}ms</span>
            </div>
            <div className="flex items-center gap-1 text-sm text-muted-foreground">
              <Rows3Icon className="size-5"/>
              <span>{queryMutation.data.results.length} rows</span>
            </div>
          </>
        )}
        <Button
          disabled={queryMutation.isPending}
          onClick={() => queryMutation.mutate()}
          size="sm"
        >
          {queryMutation.isPending && <Loader2Icon className="animate-spin"/>}
          Run Query
        </Button>
      </div>
    </div>
  )
}

import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { query } from "@/commands.ts";
import { toast } from "@/hooks/use-toast.ts";
import { DbConnectionMeta } from "@/stores.ts";
import { Textarea } from "@/components/ui/textarea.tsx";
import { Button } from "@/components/ui/button.tsx";
import { Loader2Icon, PlayIcon } from "lucide-react";
import { TableViewMain } from "@/components/project/TableView.tsx";

export function QueryView({ connection }: { connection: DbConnectionMeta }) {
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
      <div className="flex flex-col flex-1 *:rounded-none">
        <Textarea
          value={queryText}
          onChange={(e) => setQueryText(e.target.value)}
        />

        <div className="flex-1">

          {queryMutation.data != null ?
            <TableViewMain results={queryMutation.data.results}
              queryTime={queryMutation.data.queryTime} /> : queryMutation.isPending ? 'Running...' : ''}
        </div>

      </div>
      <div className="flex justify-end gap-4 p-2">
        <Button
          disabled={queryMutation.isPending}
          onClick={() => queryMutation.mutate()}
          size="sm"
        >
          {queryMutation.isPending ? <Loader2Icon className="animate-spin" /> : <PlayIcon />}
          Run Query
        </Button>
      </div>
    </div>
  )
}

import { query } from "@/commands.ts";
import { TableViewMain } from "@/components/project/TableView.tsx";
import { Button } from "@/components/ui/button.tsx";
import { Textarea } from "@/components/ui/textarea.tsx";
import type { DbConnectionMeta } from "@/stores.ts";
import { buildConnString } from "@/utils";
import { useMutation } from "@tanstack/react-query";
import { Loader2Icon, PlayIcon } from "lucide-react";
import { useState } from "react";

export function QueryView({ connection }: { connection: DbConnectionMeta }) {
  const [queryText, setQueryText] = useState("");

  const queryMutation = useMutation({
    mutationFn: async () => {
      const connString = buildConnString(connection);
      const queryTime = performance.now();
      const results = await query(connString, queryText, []);
      const endQueryTime = performance.now();
      return {
        results,
        queryTime: endQueryTime - queryTime,
      };
    },
  });

  return (
    <div className="flex flex-col h-full">
      <div className="flex flex-col flex-1 *:rounded-none">
        <Textarea
          value={queryText}
          onChange={(e) => setQueryText(e.target.value)}
        />

        <div className="flex-1">
          {
            queryMutation.status === "pending" ? (
              "Running..."
            ) : queryMutation.status === "error" ? (
              queryMutation.error.toString()
            ) : queryMutation.status === "success" ? (
              <TableViewMain
                results={queryMutation.data.results}
                queryTime={queryMutation.data.queryTime}
              />
            ) : null
          }
        </div>
      </div>
      <div className="flex justify-end gap-4 p-2">
        <Button
          disabled={queryMutation.isPending}
          onClick={() => queryMutation.mutate()}
          size="sm"
        >
          {queryMutation.isPending ? (
            <Loader2Icon className="animate-spin" />
          ) : (
            <PlayIcon />
          )}
          Run Query
        </Button>
      </div>
    </div>
  );
}

import { query } from "@/commands.ts";
import { TableViewMain } from "@/components/project/TableView.tsx";
import { Button } from "@/components/ui/button.tsx";
import { CodeEditor } from "@/components/code-editor";
import type { DbConnectionMeta } from "@/stores.ts";
import { buildConnString } from "@/utils";
import { useMutation } from "@tanstack/react-query";
import { Loader2Icon, PlayIcon, RefreshCcwIcon } from "lucide-react";
import { useState } from "react";
import { useConnection } from "@/queries/use-connection";
import { Link } from "@tanstack/react-router";

// SQLite query suggestions
const QUERY_SUGGESTIONS = [
  {
    label: "Tables",
    query: "SELECT name FROM sqlite_master WHERE type='table';",
  },
  {
    label: "Table schema",
    query: "PRAGMA table_info({table_name});",
  },
  {
    label: "All rows",
    query: "SELECT * FROM {table_name} LIMIT 100;",
  },
  {
    label: "Count rows",
    query: "SELECT COUNT(*) FROM {table_name};",
  },
  {
    label: "Search",
    query: "SELECT * FROM {table_name} WHERE {column} LIKE '%{value}%' LIMIT 50;",
  },
];

// Button colors for suggestions
const BUTTON_COLORS = [
  "border-blue-950!",
  "border-green-950!",
  "border-purple-950!",
  "border-amber-950!",
  "border-rose-950!",
];

export function SQLiteQueryView({ connection: connMeta }: { connection: DbConnectionMeta }) {
  const [queryText, setQueryText] = useState("");
  const connString = buildConnString(connMeta);

  // Use the connection hook
  const { status: connectionStatus, retry } = useConnection(connMeta.id);

  const queryMutation = useMutation({
    mutationFn: async () => {
      const queryTime = performance.now();
      const results = await query(connString, queryText, []);

      const columns = results.length > 0 ? Object.keys(results[0]).map((name, index) => ({
        index,
        name,
        type: "TEXT",
        pk: false,
      })) : [];

      const endQueryTime = performance.now();
      return {
        columns,
        results,
        queryTime: endQueryTime - queryTime,
      };
    },
  });

  // Function to apply a suggestion
  const applySuggestion = (suggestionQuery: string) => {
    setQueryText(suggestionQuery);
  };

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
      return (
        <div className="flex flex-col h-full">
          <div className="flex flex-col flex-1 *:rounded-none">
            {/* Query Suggestions */}
            <div className="flex gap-2 p-2 bg-muted/30 border-b overflow-x-auto no-scrollbar">
              <div className="flex-shrink-0 flex items-center gap-1 text-sm text-muted-foreground">
                <span>✨</span>
                <span>Suggestions:</span>
              </div>
              {QUERY_SUGGESTIONS.map((suggestion, index) => {
                return (
                  <Button
                    key={suggestion.label}
                    variant="outline"
                    size="sm"
                    className={`flex-shrink-0 h-6 text-xs rounded-full ${BUTTON_COLORS[index]}`}
                    onClick={() => applySuggestion(suggestion.query)}
                  >
                    {suggestion.label}
                  </Button>
                );
              })}
            </div>

            <CodeEditor
              value={queryText}
              onChange={setQueryText}
              language="sql"
              placeholder="Enter your SQL query..."
              className="min-h-32 font-mono"
            />

            <div className="flex-1">
              {
                queryMutation.status === "pending" ? (
                  "Running..."
                ) : queryMutation.status === "error" ? (
                  queryMutation.error.toString()
                ) : queryMutation.status === "success" ? (
                  <TableViewMain
                    columns={queryMutation.data.columns}
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
} 
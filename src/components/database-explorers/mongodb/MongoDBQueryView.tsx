import { query } from "@/commands.ts";
import { TableViewMain } from "@/components/project/TableView.tsx";
import { Button } from "@/components/ui/button.tsx";
import { CodeEditor } from "@/components/code-editor";
import type { DbConnectionMeta } from "@/stores/db-connections";
import { buildConnString } from "@/utils";
import { useMutation } from "@tanstack/react-query";
import { Loader2Icon, PlayIcon, RefreshCcwIcon } from "lucide-react";
import { useState, useEffect } from "react";
import { useConnection } from "@/queries/use-connection";
import { Link } from "@tanstack/react-router";
import { addQueryToHistory } from "@/stores/query-history";
import { useWorkspace } from "@/contexts/WorkspaceContext";

// MongoDB query suggestions
const QUERY_SUGGESTIONS = [
  {
    label: "List collections",
    query: JSON.stringify({ listCollections: 1 }, null, 2),
  },
  {
    label: "Find all",
    query: JSON.stringify({ find: "{collection}", limit: 100 }, null, 2),
  },
  {
    label: "Count documents",
    query: JSON.stringify({ count: "{collection}" }, null, 2),
  },
  {
    label: "Find with query",
    query: JSON.stringify({
      find: "{collection}",
      filter: { "{field}": "{value}" },
      limit: 10
    }, null, 2),
  },
  {
    label: "Aggregation",
    query: JSON.stringify({
      aggregate: "{collection}",
      pipeline: [
        { "$match": { "{field}": "{value}" } },
        { "$group": { "_id": "$field", "count": { "$sum": 1 } } }
      ],
      cursor: {}
    }, null, 2),
  },
];

// Button colors for suggestions
const BUTTON_COLORS = [
  "border-emerald-950!",
  "border-cyan-950!",
  "border-indigo-950!",
  "border-orange-950!",
  "border-pink-950!",
];

export function MongoDBQueryView({ connection: connMeta }: { connection: DbConnectionMeta }) {
  const { activeTab } = useWorkspace();
  const [queryText, setQueryText] = useState("");
  const connString = buildConnString(connMeta);

  // Use the connection hook
  const { status: connectionStatus, retry } = useConnection(connMeta.id);

  // Initialize with initialQuery if provided
  useEffect(() => {
    if (activeTab?.descriptor.type === "query-view" && activeTab.descriptor.initialQuery) {
      setQueryText(activeTab.descriptor.initialQuery);
    }
  }, [activeTab]);

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
      const executionTime = endQueryTime - queryTime;

      // Add query to history with metadata
      try {
        await addQueryToHistory(connMeta.id, queryText, {
          executionTime,
          resultsCount: results.length,
        });
      } catch (error) {
        console.error("Failed to add query to history:", error);
      }

      return {
        columns,
        results,
        queryTime: executionTime,
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
              language="json"
              placeholder="Enter your MongoDB query as JSON..."
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
import { useState, useCallback, useEffect, useMemo, useRef } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { cmd } from "@/commands";
import {
  PlayIcon,
  SaveIcon,
  StarIcon,
  Loader2Icon,
  XIcon,
  SquareIcon,
} from "lucide-react";
import {
  $pendingDraftQuery,
  recordHistory,
} from "@/stores/query-history-store";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { CodeEditor, type SqlSchema } from "@/components/code-editor";
import { DataTable } from "@/components/data-table";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { toast } from "sonner";
import type { SavedQuery, Engine } from "@/types/project";
import { queryKeys } from "@/lib/query-keys";
import type { ColumnDef } from "@tanstack/react-table";

interface QueryEditorProps {
  projectPath: string;
  connectionKey: string;
  engine: Engine;
  filename?: string; // If editing existing query
  onClose?: () => void;
  onSaved?: (filename: string) => void;
}

export function QueryEditor({
  projectPath,
  connectionKey,
  engine,
  filename,
  onClose,
  onSaved,
}: QueryEditorProps) {
  const queryClient = useQueryClient();
  const isNewQuery = !filename;

  // Local state
  const [queryName, setQueryName] = useState("");
  const [queryContent, setQueryContent] = useState("");
  const [description, setDescription] = useState("");
  const [favorite, setFavorite] = useState(false);
  const [paramValues, setParamValues] = useState<
    Record<string, string | number | boolean>
  >({});
  const [isDirty, setIsDirty] = useState(false);

  // For MongoDB
  const [mongoCollection, setMongoCollection] = useState("");
  const [mongoQueryType, setMongoQueryType] = useState<"find" | "aggregate">(
    "find",
  );

  // Load existing query if editing
  const savedQueryQuery = useQuery({
    queryKey: queryKeys.savedQuery(projectPath, filename ?? ""),
    queryFn: async () => {
      if (!filename) return null;
      return await cmd.getSavedQuery(projectPath, filename);
    },
    enabled: !!filename,
  });

  // Initial content for new drafts: if there's a pending draft
  // stashed in the store (e.g. from history replay or "duplicate
  // query"), drain it exactly once.
  useEffect(() => {
    if (!isNewQuery) return;
    const pending = $pendingDraftQuery.get();
    if (pending) {
      setQueryContent(pending);
      setIsDirty(true);
      $pendingDraftQuery.set(null);
    }
  }, [isNewQuery]);

  // Initialize state when query loads
  useEffect(() => {
    if (savedQueryQuery.data) {
      const q = savedQueryQuery.data;
      setQueryName(q.name);
      setDescription(q.description ?? "");
      setFavorite(q.favorite ?? false);

      if (q.sql) {
        setQueryContent(q.sql.query);
      } else if (q.mongo) {
        setMongoQueryType(q.mongo.type);
        setQueryContent(q.mongo.filter ?? q.mongo.pipeline ?? "{}");
      }

      // Initialize param values with defaults
      if (q.params) {
        const defaults: Record<string, string | number | boolean> = {};
        for (const [key, param] of Object.entries(q.params)) {
          if (param && param.default != null) {
            defaults[key] = param.default as string | number | boolean;
          }
        }
        setParamValues(defaults);
      }
    }
  }, [savedQueryQuery.data]);

  // Currently-running query token. Held in a ref so the cancel
  // handler can read the *latest* token without re-rendering — if it
  // lived in state, the onClick closure would race and cancel an
  // already-completed query in the worst case.
  const activeTokenRef = useRef<string | null>(null);
  const [executionTimeoutMs, setExecutionTimeoutMs] = useState<number>(0);

  // Execute query mutation
  const executeMutation = useMutation({
    mutationFn: async () => {
      const token = crypto.randomUUID?.() ?? String(Math.random());
      activeTokenRef.current = token;
      const startedAt = performance.now();
      const timeoutArg = executionTimeoutMs > 0 ? executionTimeoutMs : null;
      try {
        const result =
          engine === "mongodb"
            ? await cmd.executeRawMongo(
                projectPath,
                connectionKey,
                mongoCollection,
                mongoQueryType,
                queryContent,
                token,
                timeoutArg,
              )
            : await cmd.executeRawSql(
                projectPath,
                connectionKey,
                substitutedQuery,
                token,
                timeoutArg,
              );
        const duration = Math.round(performance.now() - startedAt);
        recordHistory({
          projectPath,
          connKey: connectionKey,
          query: queryContent,
          ranAt: Date.now(),
          durationMs: duration,
          rowCount: result.total_count ?? result.rows.length,
        });
        return result;
      } catch (err) {
        recordHistory({
          projectPath,
          connKey: connectionKey,
          query: queryContent,
          ranAt: Date.now(),
          durationMs: null,
          rowCount: null,
          error:
            err instanceof Error
              ? err.message.slice(0, 500)
              : String(err).slice(0, 500),
        });
        throw err;
      } finally {
        activeTokenRef.current = null;
      }
    },
    onError: (error) => {
      toast.error("Query failed", {
        description: error instanceof Error ? error.message : String(error),
      });
    },
  });

  const handleCancel = useCallback(async () => {
    const token = activeTokenRef.current;
    if (!token) return;
    try {
      await cmd.cancelQuery(token);
    } catch (err) {
      // Silently ignore — cancel is best-effort. If the query already
      // resolved before the cancel arrived, that's not a user-facing
      // error.
      console.warn("cancel_query failed", err);
    }
  }, []);

  // Save query mutation
  const saveMutation = useMutation({
    mutationFn: async () => {
      // Generate filename from query name for new queries
      const saveFilename = isNewQuery
        ? `${queryName.trim().toLowerCase().replace(/\s+/g, "_")}.query.toml`
        : filename!;

      if (!saveFilename || !saveFilename.endsWith(".query.toml")) {
        throw new Error("Invalid filename");
      }

      const query: SavedQuery = {
        name: queryName,
        connection: connectionKey,
        description: description || null,
        tags: null,
        favorite,
        params: null,
        sql: engine === "mongodb" ? null : { query: queryContent },
        mongo:
          engine === "mongodb"
            ? {
                type: mongoQueryType,
                filter: mongoQueryType === "find" ? queryContent : null,
                pipeline: mongoQueryType === "aggregate" ? queryContent : null,
              }
            : null,
      };

      await cmd.saveQuery(projectPath, saveFilename, query);
      return saveFilename;
    },
    onSuccess: (savedFilename) => {
      toast.success("Query saved");
      setIsDirty(false);
      queryClient.invalidateQueries({
        queryKey: queryKeys.savedQueries(projectPath),
      });
      onSaved?.(savedFilename);
    },
    onError: (error) => {
      toast.error("Failed to save query", {
        description: error instanceof Error ? error.message : String(error),
      });
    },
  });

  const handleContentChange = useCallback((value: string) => {
    setQueryContent(value);
    setIsDirty(true);
  }, []);

  /**
   * Substitute `:name` placeholders with the current param values.
   *
   * Intentionally simple: this is client-side string templating, not
   * real parameter binding, which means users should still treat
   * param values as untrusted input at the review stage. Once we add
   * a proper prepared-statement path this helper will be removed.
   */
  const substitutedQuery = useMemo(() => {
    if (!savedQueryQuery.data?.params) return queryContent;
    let sql = queryContent;
    for (const [name, value] of Object.entries(paramValues)) {
      const placeholder = new RegExp(`:${name}\\b`, "g");
      const serialized =
        typeof value === "string"
          ? `'${value.replace(/'/g, "''")}'`
          : String(value);
      sql = sql.replace(placeholder, serialized);
    }
    return sql;
  }, [queryContent, paramValues, savedQueryQuery.data?.params]);

  const handleExecute = () => {
    executeMutation.mutate();
  };

  const handleSave = () => {
    if (!queryName.trim()) {
      toast.error("Please enter a query name");
      return;
    }
    saveMutation.mutate();
  };

  // Build a SQL autocomplete schema from whatever we already have in
  // the React Query cache. We deliberately do NOT kick off a fresh
  // describe request here: the tree/sidebar populates the cache for
  // tables the user has browsed, and that's enough for the common
  // "type FROM, hit Tab" flow. The cache lookup is cheap and doesn't
  // introduce new N+1 request storms.
  const sqlSchema: SqlSchema | undefined = useMemo(() => {
    if (engine === "mongodb") return undefined;
    const schema: SqlSchema = {};
    const caches = queryClient.getQueriesData<unknown>({
      queryKey: queryKeys.conn.all(projectPath, connectionKey),
    });
    for (const [, data] of caches) {
      if (!data) continue;
      // SQLite objects: { name, type, ... }[]
      if (Array.isArray(data)) {
        for (const item of data as Array<{ name?: unknown }>) {
          if (
            item &&
            typeof item === "object" &&
            typeof item.name === "string"
          ) {
            schema[item.name] ??= [];
          }
        }
      }
      // Postgres tables: [{ schema, name }][]; also handled by the loop above.
      // Postgres tableDescribe: { columns: [{ name }], ... } — capture columns too.
      if (
        data &&
        typeof data === "object" &&
        "columns" in data &&
        "name" in data &&
        typeof (data as { name: unknown }).name === "string"
      ) {
        const d = data as { name: string; columns: Array<{ name: string }> };
        schema[d.name] = d.columns.map((c) => c.name);
      }
    }
    return Object.keys(schema).length > 0 ? schema : undefined;
  }, [engine, queryClient, projectPath, connectionKey]);

  // Build columns for results
  const resultColumns: ColumnDef<Record<string, unknown>>[] =
    executeMutation.data?.columns.map((col) => ({
      accessorKey: col.name,
      header: col.name,
    })) ?? [];

  // Convert rows to objects
  const resultData: Record<string, unknown>[] =
    executeMutation.data?.rows.map((row) => {
      const obj: Record<string, unknown> = {};
      executeMutation.data?.columns.forEach((col, idx) => {
        obj[col.name] = row[idx];
      });
      return obj;
    }) ?? [];

  const language = engine === "mongodb" ? "json" : "sql";

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2 border-b bg-muted/20">
        <Input
          value={queryName}
          onChange={(e) => {
            setQueryName(e.target.value);
            setIsDirty(true);
          }}
          placeholder="Query name..."
          className="h-7 text-sm font-medium flex-1 max-w-xs"
        />

        <Button
          variant="ghost"
          size="icon"
          className="size-7"
          onClick={() => {
            setFavorite(!favorite);
            setIsDirty(true);
          }}
        >
          <StarIcon
            className={`size-4 ${favorite ? "fill-yellow-500 text-yellow-500" : ""}`}
          />
        </Button>

        <div className="flex-1" />

        <Select
          value={String(executionTimeoutMs)}
          onValueChange={(v) => setExecutionTimeoutMs(Number(v))}
        >
          <SelectTrigger
            className="h-7 w-[100px] text-xs"
            title="Query timeout"
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="0">No timeout</SelectItem>
            <SelectItem value="10000">10s</SelectItem>
            <SelectItem value="30000">30s</SelectItem>
            <SelectItem value="60000">1m</SelectItem>
            <SelectItem value="300000">5m</SelectItem>
          </SelectContent>
        </Select>

        <Button
          variant="outline"
          size="sm"
          className="h-7 text-xs"
          onClick={handleSave}
          disabled={saveMutation.isPending || !queryName.trim()}
        >
          {saveMutation.isPending ? (
            <Loader2Icon className="size-3 mr-1 animate-spin" />
          ) : (
            <SaveIcon className="size-3 mr-1" />
          )}
          Save{isDirty ? " •" : ""}
        </Button>

        {executeMutation.isPending ? (
          <Button
            size="sm"
            variant="destructive"
            className="h-7 text-xs"
            onClick={handleCancel}
          >
            <SquareIcon className="size-3 mr-1" />
            Cancel
          </Button>
        ) : (
          <Button
            size="sm"
            className="h-7 text-xs"
            onClick={handleExecute}
            disabled={!queryContent.trim()}
          >
            <PlayIcon className="size-3 mr-1" />
            Run
          </Button>
        )}

        {onClose && (
          <Button
            variant="ghost"
            size="icon"
            className="size-7"
            onClick={onClose}
          >
            <XIcon className="size-4" />
          </Button>
        )}
      </div>

      {/* Main content */}
      <ResizablePanelGroup direction="vertical" className="flex-1">
        <ResizablePanel defaultSize={50} minSize={20}>
          <div className="flex flex-col h-full">
            {/* Params panel — only shown when the saved query
                declares params. Values default to whatever the
                saved-query TOML specified, so re-running a parameterized
                query is still one click. */}
            {savedQueryQuery.data?.params &&
              Object.keys(savedQueryQuery.data.params).length > 0 && (
                <div className="flex flex-wrap items-center gap-3 px-3 py-2 border-b bg-muted/10">
                  {Object.entries(savedQueryQuery.data.params).map(
                    ([name, param]) =>
                      param ? (
                        <div key={name} className="flex items-center gap-1.5">
                          <Label className="text-xs font-mono">:{name}</Label>
                          <Input
                            value={String(paramValues[name] ?? "")}
                            placeholder={
                              param.description ?? String(param.default ?? "")
                            }
                            onChange={(e) =>
                              setParamValues((prev) => ({
                                ...prev,
                                [name]:
                                  param.type === "number"
                                    ? Number(e.target.value)
                                    : param.type === "boolean"
                                      ? e.target.value === "true"
                                      : e.target.value,
                              }))
                            }
                            className="h-7 w-32 text-xs font-mono"
                          />
                        </div>
                      ) : null,
                  )}
                </div>
              )}

            {/* MongoDB-specific options */}
            {engine === "mongodb" && (
              <div className="flex items-center gap-3 px-3 py-2 border-b bg-muted/10">
                <div className="flex items-center gap-2">
                  <Label className="text-xs">Collection:</Label>
                  <Input
                    value={mongoCollection}
                    onChange={(e) => setMongoCollection(e.target.value)}
                    placeholder="collection_name"
                    className="h-7 w-40 text-xs"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <Label className="text-xs">Type:</Label>
                  <Select
                    value={mongoQueryType}
                    onValueChange={(v) =>
                      setMongoQueryType(v as "find" | "aggregate")
                    }
                  >
                    <SelectTrigger className="h-7 w-28 text-xs">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="find">Find</SelectItem>
                      <SelectItem value="aggregate">Aggregate</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              </div>
            )}

            {/* Query editor */}
            <div className="flex-1 min-h-0">
              <CodeEditor
                value={queryContent}
                onChange={handleContentChange}
                language={language}
                engine={engine}
                sqlSchema={sqlSchema}
                onRun={handleExecute}
                onSave={handleSave}
                className="h-full"
                placeholder={
                  engine === "mongodb"
                    ? mongoQueryType === "find"
                      ? '{ "field": "value" }'
                      : '[{ "$match": { } }]'
                    : "SELECT * FROM table_name"
                }
              />
            </div>
          </div>
        </ResizablePanel>

        <ResizableHandle />

        <ResizablePanel defaultSize={50} minSize={20}>
          {/* Results */}
          <div className="flex flex-col h-full">
            <div className="flex items-center justify-between px-3 py-1.5 border-b bg-muted/20">
              <span className="text-xs font-medium">Results</span>
              {executeMutation.data && (
                <span className="text-xs text-muted-foreground">
                  {executeMutation.data.total_count?.toLocaleString() ?? 0} rows
                </span>
              )}
            </div>

            <div className="flex-1 min-h-0">
              {executeMutation.isPending ? (
                <div className="flex items-center justify-center h-full">
                  <Loader2Icon className="size-5 animate-spin text-muted-foreground" />
                </div>
              ) : executeMutation.isError ? (
                <div className="flex items-center justify-center h-full px-4">
                  <p className="text-xs text-destructive text-center">
                    {executeMutation.error instanceof Error
                      ? executeMutation.error.message
                      : "Query execution failed"}
                  </p>
                </div>
              ) : executeMutation.data ? (
                <DataTable columns={resultColumns} data={resultData} />
              ) : (
                <div className="flex items-center justify-center h-full">
                  <p className="text-xs text-muted-foreground">
                    Run a query to see results
                  </p>
                </div>
              )}
            </div>
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}

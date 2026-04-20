import { useState, useCallback, useEffect } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { cmd } from "@/commands";
import { PlayIcon, SaveIcon, StarIcon, Loader2Icon, XIcon } from "lucide-react";
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
import { CodeEditor } from "@/components/code-editor";
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
  // TODO(phase2-params-history): surface param editor; currently defaults are captured but not used
  const [, setParamValues] = useState<
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

  // Execute query mutation
  const executeMutation = useMutation({
    mutationFn: async () => {
      if (engine === "mongodb") {
        return await cmd.executeRawMongo(
          projectPath,
          connectionKey,
          mongoCollection,
          mongoQueryType,
          queryContent,
        );
      } else {
        // TODO: Replace params in query
        return await cmd.executeRawSql(
          projectPath,
          connectionKey,
          queryContent,
        );
      }
    },
    onError: (error) => {
      toast.error("Query failed", {
        description: error instanceof Error ? error.message : String(error),
      });
    },
  });

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

        <Button
          size="sm"
          className="h-7 text-xs"
          onClick={handleExecute}
          disabled={executeMutation.isPending || !queryContent.trim()}
        >
          {executeMutation.isPending ? (
            <Loader2Icon className="size-3 mr-1 animate-spin" />
          ) : (
            <PlayIcon className="size-3 mr-1" />
          )}
          Run
        </Button>

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

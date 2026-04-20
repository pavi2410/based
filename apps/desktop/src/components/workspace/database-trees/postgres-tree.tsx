import { useQuery } from "@tanstack/react-query";
import { ChevronRightIcon, DatabaseIcon, TableIcon } from "lucide-react";
import { useState } from "react";
import { cmd } from "@/commands";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { queryKeys } from "@/lib/query-keys";
import type { ConnectionConfig } from "@/types/project";

interface PostgresDatabaseTreeProps {
  connKey: string;
  connConfig: ConnectionConfig;
  projectPath: string;
  onSelectTable?: (tableName: string, schema?: string) => void;
  selectedTable?: string;
  selectedSchema?: string;
}

export function PostgresDatabaseTree({
  connKey,
  projectPath,
  onSelectTable,
  selectedTable,
  selectedSchema,
}: PostgresDatabaseTreeProps) {
  const [isOpen, setIsOpen] = useState(true);

  const schemasQuery = useQuery({
    queryKey: queryKeys.conn.pgSchemas(projectPath, connKey),
    queryFn: async () => {
      return await cmd.getPostgresSchemas(projectPath, connKey);
    },
    enabled: isOpen,
  });

  return (
    <div className="py-1 space-y-0.5">
      <Collapsible open={isOpen} onOpenChange={setIsOpen}>
        <CollapsibleTrigger
          render={
            <button
              type="button"
              className="w-full flex items-center gap-1.5 px-2 h-7 text-xs hover:bg-muted/50 transition-colors"
            >
              <ChevronRightIcon
                className={`size-3 text-muted-foreground transition-transform ${isOpen ? "rotate-90" : ""}`}
              />
              <DatabaseIcon className="size-3.5 text-muted-foreground" />
              <span className="flex-1 text-left font-medium">Schemas</span>
              <span className="text-[10px] text-muted-foreground tabular-nums">
                {schemasQuery.isSuccess ? schemasQuery.data.length : "–"}
              </span>
            </button>
          }
        />
        <CollapsibleContent className="ml-4 border-l border-border/50">
          {schemasQuery.isLoading && (
            <div className="text-[11px] text-muted-foreground px-3 py-1.5">
              Loading...
            </div>
          )}
          {schemasQuery.isError && (
            <div className="text-[11px] text-destructive px-3 py-1.5">
              Failed to load
            </div>
          )}
          {schemasQuery.isSuccess && schemasQuery.data.length === 0 && (
            <div className="text-[11px] text-muted-foreground px-3 py-1.5 italic">
              None
            </div>
          )}
          {schemasQuery.isSuccess &&
            schemasQuery.data.map((schema) => (
              <PostgresSchemaGroup
                key={schema.name}
                schema={schema.name}
                connKey={connKey}
                projectPath={projectPath}
                onSelectTable={onSelectTable}
                selectedTable={selectedTable}
                selectedSchema={selectedSchema}
              />
            ))}
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}

interface PostgresSchemaGroupProps {
  schema: string;
  connKey: string;
  projectPath: string;
  onSelectTable?: (tableName: string, schema?: string) => void;
  selectedTable?: string;
  selectedSchema?: string;
}

function PostgresSchemaGroup({
  schema,
  connKey,
  projectPath,
  onSelectTable,
  selectedTable,
  selectedSchema,
}: PostgresSchemaGroupProps) {
  const [isOpen, setIsOpen] = useState(false);

  const tablesQuery = useQuery({
    queryKey: queryKeys.conn.pgTables(projectPath, connKey, schema),
    queryFn: async () => {
      return await cmd.getPostgresTables(projectPath, connKey, schema);
    },
    enabled: isOpen,
  });

  const handleTableClick = (tableName: string) => {
    onSelectTable?.(tableName, schema);
  };

  return (
    <Collapsible open={isOpen} onOpenChange={setIsOpen}>
      <CollapsibleTrigger
        render={
          <button
            type="button"
            className="w-full flex items-center gap-1.5 px-2 h-7 text-xs hover:bg-muted/50 transition-colors"
          >
            <ChevronRightIcon
              className={`size-3 text-muted-foreground transition-transform ${isOpen ? "rotate-90" : ""}`}
            />
            <DatabaseIcon className="size-3.5 text-muted-foreground" />
            <span className="flex-1 text-left font-medium">{schema}</span>
            <span className="text-[10px] text-muted-foreground tabular-nums">
              {tablesQuery.isSuccess ? tablesQuery.data.length : "–"}
            </span>
          </button>
        }
      />
      <CollapsibleContent className="ml-4 border-l border-border/50">
        {tablesQuery.isLoading && (
          <div className="text-[11px] text-muted-foreground px-3 py-1.5">
            Loading...
          </div>
        )}
        {tablesQuery.isError && (
          <div className="text-[11px] text-destructive px-3 py-1.5">
            Failed to load
          </div>
        )}
        {tablesQuery.isSuccess && tablesQuery.data.length === 0 && (
          <div className="text-[11px] text-muted-foreground px-3 py-1.5 italic">
            None
          </div>
        )}
        {tablesQuery.isSuccess &&
          tablesQuery.data.map((table) => {
            const isSelected =
              selectedTable === table.name && selectedSchema === schema;
            return (
              <button
                type="button"
                key={table.name}
                className={`w-full flex items-center gap-1.5 text-left h-6 px-3 text-[11px] truncate transition-colors ${
                  isSelected
                    ? "bg-primary/10 text-primary font-medium"
                    : "text-foreground/80 hover:bg-muted/50"
                }`}
                title={table.name}
                onClick={() => handleTableClick(table.name)}
              >
                <TableIcon className="size-3 shrink-0" />
                {table.name}
              </button>
            );
          })}
      </CollapsibleContent>
    </Collapsible>
  );
}

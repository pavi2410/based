import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import { DatabaseIcon, TableIcon, ChevronRightIcon } from "lucide-react";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { ConnectionConfig } from "@/types/project";

interface PostgresDatabaseTreeProps {
  connKey: string;
  connConfig: ConnectionConfig;
  projectPath: string;
  onSelectTable?: (tableName: string, schema?: string) => void;
  selectedTable?: string;
  selectedSchema?: string;
}

interface PostgresSchema {
  name: string;
}

interface PostgresTable {
  name: string;
  schema: string;
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
    queryKey: ["project-pg-schemas", projectPath, connKey],
    queryFn: async () => {
      const schemas = await invoke<PostgresSchema[]>("get_postgres_schemas", {
        projectPath,
        connKey,
      });
      return schemas;
    },
    enabled: isOpen,
  });

  return (
    <div className="p-2">
      <Collapsible open={isOpen} onOpenChange={setIsOpen}>
        <CollapsibleTrigger asChild>
          <Button
            variant="ghost"
            className="w-full justify-start gap-2 px-2 h-8"
          >
            <ChevronRightIcon
              className={`size-4 transition-transform ${isOpen ? "rotate-90" : ""}`}
            />
            <DatabaseIcon className="size-4" />
            <span className="flex-1 text-left text-sm">Schemas</span>
            <Badge variant="outline" className="text-xs">
              {schemasQuery.isSuccess ? schemasQuery.data.length : 0}
            </Badge>
          </Button>
        </CollapsibleTrigger>
        <CollapsibleContent className="ml-6 mt-1 space-y-1">
          {schemasQuery.isLoading && (
            <div className="text-xs text-muted-foreground px-2 py-1">
              Loading schemas...
            </div>
          )}
          {schemasQuery.isError && (
            <div className="text-xs text-destructive px-2 py-1">
              Failed to load schemas
            </div>
          )}
          {schemasQuery.isSuccess && schemasQuery.data.length === 0 && (
            <div className="text-xs text-muted-foreground px-2 py-1">
              No schemas found
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
    queryKey: ["project-pg-tables", projectPath, connKey, schema],
    queryFn: async () => {
      const tables = await invoke<PostgresTable[]>("get_postgres_tables", {
        projectPath,
        connKey,
        schema,
      });
      return tables;
    },
    enabled: isOpen,
  });

  const handleTableClick = (tableName: string) => {
    onSelectTable?.(tableName, schema);
  };

  return (
    <Collapsible open={isOpen} onOpenChange={setIsOpen}>
      <CollapsibleTrigger asChild>
        <Button
          variant="ghost"
          className="w-full justify-start gap-2 px-2 h-8"
        >
          <ChevronRightIcon
            className={`size-4 transition-transform ${isOpen ? "rotate-90" : ""}`}
          />
          <DatabaseIcon className="size-4" />
          <span className="flex-1 text-left text-sm">{schema}</span>
          <Badge variant="outline" className="text-xs">
            {tablesQuery.isSuccess ? tablesQuery.data.length : 0}
          </Badge>
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent className="ml-6 mt-1 space-y-1">
        {tablesQuery.isLoading && (
          <div className="text-xs text-muted-foreground px-2 py-1">
            Loading tables...
          </div>
        )}
        {tablesQuery.isError && (
          <div className="text-xs text-destructive px-2 py-1">
            Failed to load tables
          </div>
        )}
        {tablesQuery.isSuccess && tablesQuery.data.length === 0 && (
          <div className="text-xs text-muted-foreground px-2 py-1">
            No tables found
          </div>
        )}
        {tablesQuery.isSuccess &&
          tablesQuery.data.map((table) => {
            const isSelected = selectedTable === table.name && selectedSchema === schema;
            return (
              <Button
                key={table.name}
                variant={isSelected ? "secondary" : "ghost"}
                className="w-full justify-start gap-2 h-7 px-2 text-xs font-normal"
                title={table.name}
                onClick={() => handleTableClick(table.name)}
              >
                <TableIcon className="size-3" />
                {table.name}
              </Button>
            );
          })}
      </CollapsibleContent>
    </Collapsible>
  );
}

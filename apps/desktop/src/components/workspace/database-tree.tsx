import type { ConnectionConfig } from "@/types/project";
import { SQLiteDatabaseTree } from "./database-trees/sqlite-tree";
import { MongoDBDatabaseTree } from "./database-trees/mongodb-tree";
import { PostgresDatabaseTree } from "./database-trees/postgres-tree";

interface DatabaseTreeProps {
  connKey: string;
  connConfig: ConnectionConfig;
  projectPath: string;
  onSelectTable?: (tableName: string, schema?: string) => void;
  selectedTable?: string;
  selectedSchema?: string;
}

export function DatabaseTree({
  connKey,
  connConfig,
  projectPath,
  onSelectTable,
  selectedTable,
  selectedSchema,
}: DatabaseTreeProps) {
  switch (connConfig.engine) {
    case "sqlite":
      return (
        <SQLiteDatabaseTree
          connKey={connKey}
          connConfig={connConfig}
          projectPath={projectPath}
          onSelectTable={onSelectTable}
          selectedTable={selectedTable}
        />
      );
    case "mongodb":
      return (
        <MongoDBDatabaseTree
          connKey={connKey}
          connConfig={connConfig}
          projectPath={projectPath}
          onSelectTable={onSelectTable}
          selectedTable={selectedTable}
        />
      );
    case "postgres":
      return (
        <PostgresDatabaseTree
          connKey={connKey}
          connConfig={connConfig}
          projectPath={projectPath}
          onSelectTable={onSelectTable}
          selectedTable={selectedTable}
          selectedSchema={selectedSchema}
        />
      );
    default:
      return (
        <div className="p-4 text-sm text-muted-foreground">
          Unknown database engine: {connConfig.engine}
        </div>
      );
  }
}

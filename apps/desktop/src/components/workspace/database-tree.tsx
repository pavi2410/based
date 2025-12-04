import type { ConnectionConfig } from "@/types/project";
import { SQLiteDatabaseTree } from "./database-trees/sqlite-tree";
import { MongoDBDatabaseTree } from "./database-trees/mongodb-tree";

interface DatabaseTreeProps {
  connKey: string;
  connConfig: ConnectionConfig;
  projectPath: string;
}

export function DatabaseTree({
  connKey,
  connConfig,
  projectPath,
}: DatabaseTreeProps) {
  switch (connConfig.engine) {
    case "sqlite":
      return (
        <SQLiteDatabaseTree
          connKey={connKey}
          connConfig={connConfig}
          projectPath={projectPath}
        />
      );
    case "mongodb":
      return (
        <MongoDBDatabaseTree
          connKey={connKey}
          connConfig={connConfig}
          projectPath={projectPath}
        />
      );
    case "postgres":
      return (
        <div className="p-4 text-sm text-muted-foreground">
          PostgreSQL explorer coming in Phase 9
        </div>
      );
    default:
      return (
        <div className="p-4 text-sm text-muted-foreground">
          Unknown database engine: {connConfig.engine}
        </div>
      );
  }
}

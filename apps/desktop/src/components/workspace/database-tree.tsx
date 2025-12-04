import type { DatabaseConfig } from "@/types/project";
import { SQLiteDatabaseTree } from "./database-trees/sqlite-tree";
import { MongoDBDatabaseTree } from "./database-trees/mongodb-tree";

interface DatabaseTreeProps {
  dbKey: string;
  dbConfig: DatabaseConfig;
  projectPath: string;
  environment: string;
}

export function DatabaseTree({
  dbKey,
  dbConfig,
  projectPath,
  environment,
}: DatabaseTreeProps) {
  switch (dbConfig.type) {
    case "sqlite":
      return (
        <SQLiteDatabaseTree
          dbKey={dbKey}
          dbConfig={dbConfig}
          projectPath={projectPath}
          environment={environment}
        />
      );
    case "mongodb":
      return (
        <MongoDBDatabaseTree
          dbKey={dbKey}
          dbConfig={dbConfig}
          projectPath={projectPath}
          environment={environment}
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
          Unknown database type: {dbConfig.type}
        </div>
      );
  }
}

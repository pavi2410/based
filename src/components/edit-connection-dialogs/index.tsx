import { EditSQLiteConnectionDialog } from "./sqlite";
import { EditMongoDBConnectionDialog } from "./mongodb";
import { type ConnectionMeta } from "@/stores/db-connections";

export { EditSQLiteConnectionDialog, EditMongoDBConnectionDialog };

// Utility function to get the correct edit dialog based on connection type
export function EditConnectionDialog({
  connection,
  trigger,
}: {
  connection: ConnectionMeta,
  trigger: React.ReactNode
}) {
  if (connection.dbType === 'sqlite') {
    return (
      <EditSQLiteConnectionDialog connection={connection} trigger={trigger} />
    );
  } else {
    return (
      <EditMongoDBConnectionDialog connection={connection} trigger={trigger} />
    );
  }
} 
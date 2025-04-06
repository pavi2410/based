import { load } from "./commands";
import type { ConnectionVariables } from "./stores/index";

export function baseName(path: string) {
  return path.replace(/^.+[\/\\]/, "");
}

export function buildConnString(connection: ConnectionVariables) {
  if (connection.dbType === "sqlite") {
    return `sqlite:${connection.filePath}`;
  } else if (connection.dbType === "mongodb") {
    return connection.connectionString;
  } else {
    throw new Error("Unsupported DB type");
  }
}

export async function testConnection(connString: string) {
  try {
    await load(connString);
    return true;
  } catch (error) {
    console.error('Error loading connection:', error);
    return false;
  }
}
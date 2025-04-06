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

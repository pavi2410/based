import type { DbConnectionMeta } from "./stores";

export function baseName(path: string) {
  return path.replace(/^.+[\/\\]/, "");
}

export function buildConnString(connection: DbConnectionMeta) {
  if (connection.dbType === "sqlite") {
    return `sqlite:${connection.filePath}`;
  } else if (connection.dbType === "mongodb") {
    return connection.filePath; // MongoDB connection string is already in the correct format
  } else {
    throw new Error("Unsupported DB type");
  }
}

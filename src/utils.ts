import type { DbConnectionMeta } from "./stores";

export function baseName(path: string) {
  return path.replace(/^.+[\/\\]/, "");
}

export function buildConnString(connection: DbConnectionMeta) {
  if (connection.dbType !== "sqlite") {
    throw new Error("Unsupported DB type");
  }
  return `sqlite:${connection.filePath}`;
}

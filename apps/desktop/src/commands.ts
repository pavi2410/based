/**
 * Thin helpers around the auto-generated `commands` object from `bindings.ts`.
 *
 * tauri-specta emits every command as a function that returns
 * `Promise<Result<T, string>>`. We unwrap that here so callers can simply
 * `await` a Promise<T> and rely on the usual throw-on-error flow.
 */
import { commands as raw } from "./bindings";

type RawCommands = typeof raw;

type UnwrapResult<T> = T extends Promise<
  { status: "ok"; data: infer D } | { status: "error"; error: unknown }
>
  ? Promise<D>
  : T extends Promise<{ status: "ok"; data: infer D }>
    ? Promise<D>
    : never;

export type Cmd = {
  [K in keyof RawCommands]: (
    ...args: Parameters<RawCommands[K]>
  ) => UnwrapResult<ReturnType<RawCommands[K]>>;
};

function unwrap<T>(
  p: Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>,
): Promise<T> {
  return p.then((r) => {
    if (r.status === "ok") return r.data;
    throw new Error(r.error);
  });
}

/**
 * Typed command surface for every Rust command decorated with `#[specta::specta]`.
 * Each method returns a Promise<T> that throws on error.
 */
export const cmd: Cmd = new Proxy({} as Cmd, {
  get(_, key: string) {
    const fn = (
      raw as unknown as Record<
        string,
        (
          ...a: unknown[]
        ) => Promise<
          { status: "ok"; data: unknown } | { status: "error"; error: string }
        >
      >
    )[key];
    if (!fn) throw new Error(`Unknown command: ${key}`);
    return (...args: unknown[]) => unwrap(fn(...args));
  },
});

export type {
  ProjectConfig,
  ConnectionConfig,
  ConnectionInfo,
  Engine,
  SecretValue,
  ProjectSettings,
  SavedQuery,
  SqlQuery,
  MongoQuery,
  MongoQueryType,
  QueryParameter,
  QueryParamType,
  QuerySummary,
  QueryResult,
  ColumnInfo,
  BrowseOptions,
  SQLiteObject,
  MongoDBCollection,
  PostgresSchema,
  PostgresTable,
  ProjectAddress,
  ConnectionAddress,
  TabAddress,
  TableDescription,
  ColumnDescription,
  IndexDescription,
  ForeignKeyDescription,
} from "./bindings";

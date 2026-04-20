/**
 * Centralized React Query key builders.
 *
 * Every component that reads from the backend should go through this
 * file instead of inlining `queryKey: ['table-data', ...]` literals.
 * That gives us two properties:
 *
 *   1. **Safe invalidation** — mutation hooks can target a shared
 *      prefix (e.g. `keys.conn.all(project, conn)`) and be sure they
 *      match the readers. Today the browse and describe queries each
 *      pick their own prefix; the row-mutations hook only hits two of
 *      them, which is why "insert a row and the tree doesn't show the
 *      new count" is a current annoyance.
 *
 *   2. **Typed keys** — `as const` tuples mean TypeScript catches a
 *      misspelled prefix at build time instead of silently failing to
 *      match at runtime.
 *
 * Shape: everything that scopes to a single connection lives under
 * `["conn", projectPath, connKey, ...]` so callers can invalidate the
 * whole connection (e.g. after a reconnect) with one call.
 */

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Key = readonly (
  | string
  | number
  | boolean
  | null
  | Record<string, unknown>
)[];

export const queryKeys = {
  /** Workspace-scoped (project root, not tied to any connection). */
  projectConfig: (projectPath: string) =>
    ["project-config", projectPath] as const,

  savedQueries: (projectPath: string) =>
    ["saved-queries", projectPath] as const,

  savedQuery: (projectPath: string, filename: string) =>
    ["saved-query", projectPath, filename] as const,

  /** Connection-scoped keys. Share the `["conn", project, key]` prefix. */
  conn: {
    /**
     * Root prefix for a single connection. Pass this to
     * `queryClient.invalidateQueries({ queryKey: ... })` to nuke every
     * cached read for that connection.
     */
    all: (projectPath: string, connKey: string) =>
      ["conn", projectPath, connKey] as const,

    /** SQLite `sqlite_master`-style object listing (type = table | view | index | trigger). */
    sqliteObjects: (projectPath: string, connKey: string, type: string) =>
      ["conn", projectPath, connKey, "sqlite-objects", type] as const,

    /** MongoDB collection list. */
    mongoCollections: (projectPath: string, connKey: string) =>
      ["conn", projectPath, connKey, "mongo-collections"] as const,

    /** Postgres schemas. */
    pgSchemas: (projectPath: string, connKey: string) =>
      ["conn", projectPath, connKey, "pg-schemas"] as const,

    /** Postgres tables in a schema. */
    pgTables: (projectPath: string, connKey: string, schema: string) =>
      ["conn", projectPath, connKey, "pg-tables", schema] as const,

    /**
     * Prefix for _all_ browse/table-data queries on this connection.
     * Used by `useRowMutations` to invalidate every page of data after
     * an insert/update/delete.
     */
    tableDataAll: (projectPath: string, connKey: string) =>
      ["conn", projectPath, connKey, "table-data"] as const,

    /** Single page of table data (browse). */
    tableData: (
      projectPath: string,
      connKey: string,
      selector: {
        schema: string | null;
        name: string;
        page: number;
        // `sorting` and `filters` are passed through as plain JSON so
        // React Query can structurally hash them; callers shouldn't
        // need to pre-serialize.
        sorting: unknown;
        filters: unknown;
      },
    ) => ["conn", projectPath, connKey, "table-data", selector] as const,

    /** Prefix for _all_ describe queries on this connection. */
    tableDescribeAll: (projectPath: string, connKey: string) =>
      ["conn", projectPath, connKey, "describe"] as const,

    /** Single describe_table / describe_collection payload. */
    tableDescribe: (
      projectPath: string,
      connKey: string,
      engine: string,
      schema: string | null | undefined,
      name: string,
    ) =>
      [
        "conn",
        projectPath,
        connKey,
        "describe",
        engine,
        schema ?? null,
        name,
      ] as const,
  },
} satisfies Record<string, unknown>;

// Sanity export so no key builder is accidentally left out of a
// widened type. (`satisfies` above does most of the work; this nudges
// unused-import warnings into being real.)
export type QueryKey = Key;

/**
 * Project types re-exported from the auto-generated tauri-specta bindings.
 *
 * These types are the single source of truth and are regenerated whenever
 * Rust command signatures or payload shapes change (`cargo test specta_export`
 * in `apps/desktop/src-tauri` or just `bun tauri dev`).
 */
export type {
  ProjectConfig,
  ConnectionConfig,
  ConnectionInfo,
  ConnectionId,
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
} from "@/bindings";

/**
 * Front-end-only wrapper around a project row pulled from the recents list.
 */
export type Project = {
  path: string;
  config: import("@/bindings").ProjectConfig;
  lastOpened: number;
};

/**
 * UI-only per-project state; not serialized through tauri-specta.
 */
export type ProjectState = {
  activeConnection: string;
  openQueries: string[];
  uiState: {
    sidebarCollapsed: boolean;
    activeTab: string;
    explorerExpanded: string[];
  };
};

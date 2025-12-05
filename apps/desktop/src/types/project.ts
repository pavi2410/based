/**
 * Project configuration types for Based
 * Corresponds to .based/config.toml structure
 */

export type ProjectConfig = {
  version: number;
  name: string;
  description?: string;
  connection: Record<string, ConnectionConfig>;
  settings?: ProjectSettings;
}

export type Engine = 'sqlite' | 'mongodb' | 'postgres';

export type SecretValue =
  | { env: string }
  | { value: string }
  | { file: string }
  | string; // Literal value

export type ConnectionConfig = {
  label?: string;
  engine: Engine;
  group?: string;
  disabled?: boolean;
  order?: number;
  color?: string;
  icon?: string;

  // SQLite fields
  file?: string;
  readonly?: boolean;

  // MongoDB fields
  url?: SecretValue;

  // PostgreSQL fields
  host?: string;
  port?: number;
  database?: string;
  username?: string;
  password?: SecretValue;
  ssl?: boolean;
}

export type ProjectSettings = {
  queryTimeout?: number;
  maxResultRows?: number;
  enableQueryCache?: boolean;
  cacheTTL?: number;
}

/**
 * Saved query file (.query.toml) structure
 */
export type SavedQuery = {
  // File info (not in TOML, added by backend)
  filename: string;       // e.g., "recent_orders.query.toml"
  
  // Metadata
  name: string;
  connection: string;     // Connection key from config.toml
  description?: string;
  tags?: string[];
  favorite?: boolean;
  
  // Parameters
  params?: Record<string, QueryParameter>;
  
  // Query content (one of these based on engine)
  sql?: SqlQuery;
  mongo?: MongoQuery;
}

export type SqlQuery = {
  query: string;
}

export type MongoQuery = {
  type: 'find' | 'aggregate';
  filter?: string;        // JSON string for find queries
  pipeline?: string;      // JSON string for aggregation pipeline
}

export type QueryParameter = {
  type: 'string' | 'number' | 'date' | 'boolean' | 'select';
  default?: string | number | boolean;
  description?: string;
  options?: string[];     // For select type
}

/**
 * Query execution result
 */
export type QueryResult = {
  columns: { name: string; data_type: string }[];
  rows: unknown[][];
  total_count: number | null;
  execution_time_ms?: number;
}

/**
 * Summary info for listing queries (without full content)
 */
export type QuerySummary = {
  filename: string;
  name: string;
  connection: string;
  description?: string;
  tags?: string[];
  favorite?: boolean;
}

export type Project = {
  path: string;
  config: ProjectConfig;
  lastOpened: number;
}

export type ProjectState = {
  activeConnection: string;
  openQueries: string[];
  uiState: {
    sidebarCollapsed: boolean;
    activeTab: string;
    explorerExpanded: string[];
  };
}

/**
 * Connection info returned from the backend registry.
 * Contains the stable connection ID and metadata.
 */
export type ConnectionInfo = {
  id: string;           // Stable hash-based ID
  project_path: string;
  conn_key: string;     // Original key from config
  engine: Engine;
  label?: string;
}

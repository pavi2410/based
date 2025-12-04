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

export type QueryFile = {
  path: string;           // Relative to .based/queries/
  name: string;
  connection: string;     // Connection key
  content: string;
  metadata: QueryMetadata;
}

export type QueryMetadata = {
  name: string;
  connection: string;     // Connection key
  description?: string;
  tags?: string[];
  parameters?: QueryParameter[];
  favorite?: boolean;
}

export type QueryParameter = {
  name: string;
  type: 'string' | 'number' | 'date' | 'boolean' | 'select';
  default?: any;
  description?: string;
  options?: string[];  // For select type
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

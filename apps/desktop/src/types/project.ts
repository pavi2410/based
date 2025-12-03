/**
 * Project configuration types for Based
 * Corresponds to .based/config.toml structure
 */

export type ProjectConfig = {
  version: string;
  name: string;
  description?: string;
  databases: Record<string, DatabaseConfig>;
  environments: EnvironmentConfig;
  settings?: ProjectSettings;
}

export type DatabaseConfig = {
  name: string;
  type: 'sqlite' | 'mongodb' | 'postgres';
  connection: ConnectionConfig;
  description?: string;
  environments?: Record<string, Partial<DatabaseConfig>>;
}

export type ConnectionConfig = {
  // SQLite
  path?: string;

  // MongoDB
  url?: string;

  // PostgreSQL
  host?: string;
  port?: number;
  database?: string;
  username?: string;
  password?: string;
  sslmode?: 'disable' | 'require' | 'verify-ca' | 'verify-full';
}

export type EnvironmentConfig = {
  default: string;
  available: string[];
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
  database: string;
  content: string;
  metadata: QueryMetadata;
}

export type QueryMetadata = {
  name: string;
  database: string;
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
  activeDatabase: string;
  activeEnvironment: string;
  openQueries: string[];
  uiState: {
    sidebarCollapsed: boolean;
    activeTab: string;
    explorerExpanded: string[];
  };
}

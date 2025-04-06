import { LazyStore } from '@tauri-apps/plugin-store';

// Create a single store instance to be shared across all store modules
export const store = new LazyStore('settings.json');

// Common store keys that can be shared across modules
export const STORE_KEYS = {
  DB_CONN_META: 'db_conn_meta',
  QUERY_HISTORY: 'query_history',
}; 
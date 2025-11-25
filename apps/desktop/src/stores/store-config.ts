import { LazyStore } from '@tauri-apps/plugin-store';

// Create a single store instance to be shared across all store modules
export const store = new LazyStore('store.json');

// Common store keys that can be shared across modules
export const STORE_KEYS = {
  CONN_META: 'conn_meta',
  QUERY_HISTORY: 'query_history',
  SETTINGS: 'settings',
}; 
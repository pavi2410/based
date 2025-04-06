import { invoke } from '@tauri-apps/api/core';

export async function load(db: string) {
  return await invoke<string>('load', { db });
}

export async function close(db?: string) {
  return await invoke<boolean>('close', { db });
}

export async function query<T>(db: string, query: string, values: Array<any>) {
  return await invoke<{ db: string; result: T }>('query', { db, query, values });
}

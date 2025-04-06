import { LazyStore } from '@tauri-apps/plugin-store';

const store = new LazyStore('settings.json');

export type DbConnectionMeta = {
  id: string;
  createdAt: number;
  updatedAt: number;

  dbType: 'sqlite' | 'mongodb';
  filePath: string;

  groupName: string;
}

type CoreFields = 'id' | 'createdAt' | 'updatedAt';

const STORE_KEYS = {
  DB_CONN_META: 'db_conn_meta',
}

export async function getConnections() {
  return (await store.get<DbConnectionMeta[]>(STORE_KEYS.DB_CONN_META)) ?? [];
}

export async function getConnection(connectionId: string) {
  const connections = await getConnections();
  return connections.find(connection => connection.id === connectionId);
}

export async function addConnection(connMeta: Omit<DbConnectionMeta, CoreFields>) {
  const newId = crypto.randomUUID();
  const connections = await getConnections();
  await store.set(STORE_KEYS.DB_CONN_META, [
    ...connections,
    {
      ...connMeta,
      id: newId,
      createdAt: Date.now(),
      updatedAt: Date.now(),
    }
  ]);
  return newId;
}

export async function updateConnection(connectionId: string, connectionMeta: DbConnectionMeta) {
  const connections = await getConnections();
  const connIndex = connections.findIndex(conn => conn.id === connectionId);
  if (connIndex === -1) {
    throw new Error('Project not found');
  }
  connectionMeta.updatedAt = Date.now();
  connections[connIndex] = connectionMeta;
  await store.set(STORE_KEYS.DB_CONN_META, connections);
}

export async function removeConnection(connectionId: string) {
  const connections = await getConnections();
  await store.set(STORE_KEYS.DB_CONN_META, connections.filter(connection => connection.id !== connectionId));
}
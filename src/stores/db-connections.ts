import { store, STORE_KEYS } from './store-config';

type BaseConnectionMeta = {
  id: string;
  createdAt: number;
  updatedAt: number;
  tags: string[];
}

type SqliteConnectionVariables = {
  dbType: 'sqlite';
  filePath: string;
}

type MongoDBConnectionVariables = {
  dbType: 'mongodb';
  connectionString: string;
}

export type ConnectionVariables =
  | SqliteConnectionVariables
  | MongoDBConnectionVariables;

export type SqliteConnectionMeta = BaseConnectionMeta & SqliteConnectionVariables;

export type MongoDBConnectionMeta = BaseConnectionMeta & MongoDBConnectionVariables;

export type ConnectionMeta =
  | SqliteConnectionMeta
  | MongoDBConnectionMeta;

type AutoFields = 'id' | 'createdAt' | 'updatedAt';
type BaseVariables = Omit<BaseConnectionMeta, AutoFields>;
export type EditableFields = ConnectionVariables & BaseVariables;

export async function getConnections() {
  return (await store.get<ConnectionMeta[]>(STORE_KEYS.CONN_META)) ?? [];
}

export async function getConnection(connectionId: string) {
  const connections = await getConnections();
  return connections.find(connection => connection.id === connectionId);
}

export async function addConnection(fields: EditableFields) {
  const newId = crypto.randomUUID();
  const connections = await getConnections();

  const newConnection: ConnectionMeta = {
    ...fields,
    id: newId,
    createdAt: Date.now(),
    updatedAt: Date.now(),
  };

  await store.set(STORE_KEYS.CONN_META, [
    ...connections,
    newConnection,
  ]);
  return newId;
}

export async function updateConnection(connectionId: string, fields: EditableFields) {
  const connections = await getConnections();
  const connIndex = connections.findIndex(conn => conn.id === connectionId);
  if (connIndex === -1) {
    throw new Error('Connection not found');
  }

  const existingConnection = connections[connIndex];

  let updatedConnection: ConnectionMeta;
  if (existingConnection.dbType === 'sqlite' && fields.dbType === 'sqlite') {
    updatedConnection = {
      ...existingConnection,
      ...fields,
      updatedAt: Date.now(),
    } as SqliteConnectionMeta;
  } else if (existingConnection.dbType === 'mongodb' && fields.dbType === 'mongodb') {
    updatedConnection = {
      ...existingConnection,
      ...fields,
      updatedAt: Date.now(),
    } as MongoDBConnectionMeta;
  } else {
    throw new Error('Cannot update connection type');
  }

  connections[connIndex] = updatedConnection;
  await store.set(STORE_KEYS.CONN_META, connections);
}

export async function removeConnection(connectionId: string) {
  const connections = await getConnections();
  await store.set(STORE_KEYS.CONN_META, connections.filter(connection => connection.id !== connectionId));
} 
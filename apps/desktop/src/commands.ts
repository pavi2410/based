import { invoke } from '@tauri-apps/api/core';
import type { ConnectionInfo } from '@/types/project';

/**
 * Connect to a project database and get its stable connection ID.
 * If already connected, returns the existing ID.
 */
export async function connectProjectDb(projectPath: string, connKey: string): Promise<string> {
  return await invoke<string>('connect_project_db', { projectPath, connKey });
}

/**
 * Get connection info by ID.
 */
export async function getConnectionInfo(connId: string): Promise<ConnectionInfo> {
  return await invoke<ConnectionInfo>('get_connection_info', { connId });
}

/**
 * Close a specific connection.
 */
export async function closeConnection(projectPath: string, connKey: string): Promise<void> {
  return await invoke<void>('close_connection', { projectPath, connKey });
}

/**
 * Close all connections for a project.
 */
export async function closeProjectConnections(projectPath: string): Promise<void> {
  return await invoke<void>('close_project_connections', { projectPath });
}

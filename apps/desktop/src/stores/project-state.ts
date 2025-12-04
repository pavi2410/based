import { atom } from "nanostores";
import { persistentAtom } from "@nanostores/persistent";
import { toast } from "sonner";
import type { ProjectConfig } from "@/types/project";
import { connectProjectDb, closeConnection } from "@/commands";

/**
 * Per-project state management using nanostores
 * Tracks active connection and UI state for each project
 */

export interface RecentProject {
  path: string;
  name: string;
  lastOpened: string; // ISO timestamp
}

export type ConnectionStatus = "connected" | "disconnected" | "connecting" | "error";

// Connection stats for timing info
export interface ConnectionStats {
  connectedAt: string; // ISO timestamp
  connectionTimeMs: number; // Time taken to establish connection
}

// Active connection key for the current project (config key like "dev", "prod")
export const $activeConnection = atom<string | null>(null);

// Active connection ID (stable hash-based ID from backend)
export const $activeConnectionId = atom<string | null>(null);

// Current project path
export const $projectPath = atom<string | null>(null);

// Current project configuration
export const $projectConfig = atom<ProjectConfig | null>(null);

// Connection status for the active connection
export const $connectionStatus = atom<ConnectionStatus>("disconnected");

// Connection stats (timing info)
export const $connectionStats = atom<ConnectionStats | null>(null);

// Sidebar visibility
export const $sidebarVisible = atom<boolean>(true);

// Selected table/collection for data viewing
export interface SelectedObject {
  type: "table" | "view" | "collection";
  name: string;
  schema?: string; // For PostgreSQL
}
export const $selectedObject = atom<SelectedObject | null>(null);

// Recent projects (persisted to localStorage)
export const $recentProjects = persistentAtom<RecentProject[]>(
  "based:recent-projects",
  [],
  {
    encode: JSON.stringify,
    decode: JSON.parse,
  },
);

// Actions
export function setActiveConnection(connKey: string) {
  $activeConnection.set(connKey);
  $activeConnectionId.set(null); // Reset ID until connected
  $connectionStatus.set("disconnected");
}

export function setActiveConnectionId(connId: string) {
  $activeConnectionId.set(connId);
}

export function setProjectPath(path: string) {
  $projectPath.set(path);
}

export function setProjectConfig(config: ProjectConfig | null) {
  $projectConfig.set(config);
}

export function setConnectionStatus(status: ConnectionStatus) {
  $connectionStatus.set(status);
}

export function setConnectionStats(stats: ConnectionStats | null) {
  $connectionStats.set(stats);
}

export function toggleSidebar() {
  $sidebarVisible.set(!$sidebarVisible.get());
}

export function selectObject(obj: SelectedObject | null) {
  $selectedObject.set(obj);
}

/**
 * Disconnect the current connection and reset to empty state.
 */
export async function disconnectConnection() {
  const projectPath = $projectPath.get();
  const connKey = $activeConnection.get();

  // Close the backend connection if we have one
  if (projectPath && connKey) {
    try {
      await closeConnection(projectPath, connKey);
    } catch (error) {
      toast.error("Failed to disconnect", {
        description: error instanceof Error ? error.message : String(error),
      });
      return;
    }
  }

  // Reset frontend state only on success
  $activeConnection.set(null);
  $activeConnectionId.set(null);
  $connectionStatus.set("disconnected");
  $connectionStats.set(null);
  $selectedObject.set(null);
}

export function addRecentProject(project: RecentProject) {
  const current = $recentProjects.get();
  // Remove existing entry for this path
  const filtered = current.filter((p) => p.path !== project.path);
  // Add to front, limit to 10 recent projects
  $recentProjects.set([project, ...filtered].slice(0, 10));
}

export function removeRecentProject(projectPath: string) {
  const current = $recentProjects.get();
  $recentProjects.set(current.filter((p) => p.path !== projectPath));
}

/**
 * Switch to a new connection and establish the connection.
 * Returns the connection ID on success.
 */
export async function switchConnection(connKey: string): Promise<string | null> {
  const projectPath = $projectPath.get();
  if (!projectPath) {
    console.error("No project path set");
    return null;
  }

  setActiveConnection(connKey);
  setConnectionStatus("connecting");
  setConnectionStats(null);

  const startTime = performance.now();

  try {
    const connId = await connectProjectDb(projectPath, connKey);
    const connectionTimeMs = Math.round(performance.now() - startTime);
    
    setActiveConnectionId(connId);
    setConnectionStatus("connected");
    setConnectionStats({
      connectedAt: new Date().toISOString(),
      connectionTimeMs,
    });
    return connId;
  } catch (error) {
    console.error("Failed to connect:", error);
    setConnectionStatus("error");
    setConnectionStats(null);
    return null;
  }
}

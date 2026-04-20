import { persistentAtom } from "@nanostores/persistent";
import { atom } from "nanostores";
import { toast } from "sonner";
import { cmd } from "@/commands";
import type { ProjectConfig } from "@/types/project";

/**
 * Per-project state management using nanostores
 * Tracks active connection and UI state for each project
 */

export interface RecentProject {
  path: string;
  name: string;
  lastOpened: string; // ISO timestamp
}

export type ConnectionStatus =
  | "connected"
  | "disconnected"
  | "connecting"
  | "error";

// Connection stats for timing info
export interface ConnectionStats {
  connectedAt: string; // ISO timestamp
  connectionTimeMs: number; // Time taken to establish connection
}

// Grouped connection state
export interface ConnectionState {
  connKey: string | null; // Config key like "dev", "prod"
  connId: string | null; // Backend connection ID (stable hash)
  status: ConnectionStatus;
  stats: ConnectionStats | null;
}

const initialConnectionState: ConnectionState = {
  connKey: null,
  connId: null,
  status: "disconnected",
  stats: null,
};

// Connection state (grouped)
export const $connection = atom<ConnectionState>(initialConnectionState);

// Current project path
export const $projectPath = atom<string | null>(null);

// Current project configuration
export const $projectConfig = atom<ProjectConfig | null>(null);

// Sidebar visibility
export const $sidebarVisible = atom<boolean>(true);

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
export function setProjectPath(path: string) {
  $projectPath.set(path);
}

export function setProjectConfig(config: ProjectConfig | null) {
  $projectConfig.set(config);
}

export function toggleSidebar() {
  $sidebarVisible.set(!$sidebarVisible.get());
}

/**
 * Disconnect the current connection and reset to empty state.
 */
export async function disconnectConnection() {
  const projectPath = $projectPath.get();
  const { connKey } = $connection.get();

  // Close the backend connection if we have one
  if (projectPath && connKey) {
    try {
      await cmd.closeConnection(projectPath, connKey);
    } catch (error) {
      toast.error("Failed to disconnect", {
        description: error instanceof Error ? error.message : String(error),
      });
      return;
    }
  }

  // Reset connection state
  $connection.set(initialConnectionState);
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
export async function switchConnection(
  connKey: string,
): Promise<string | null> {
  const projectPath = $projectPath.get();
  if (!projectPath) {
    console.error("No project path set");
    return null;
  }

  // Set connecting state
  $connection.set({
    connKey,
    connId: null,
    status: "connecting",
    stats: null,
  });

  const startTime = performance.now();

  try {
    const connId = await cmd.connectProjectDb(projectPath, connKey);
    const connectionTimeMs = Math.round(performance.now() - startTime);

    // Set connected state atomically
    $connection.set({
      connKey,
      connId,
      status: "connected",
      stats: {
        connectedAt: new Date().toISOString(),
        connectionTimeMs,
      },
    });
    return connId;
  } catch (error) {
    console.error("Failed to connect:", error);
    $connection.set({
      connKey,
      connId: null,
      status: "error",
      stats: null,
    });
    return null;
  }
}

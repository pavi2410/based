import { atom } from "nanostores";
import { persistentAtom } from "@nanostores/persistent";
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

export type ConnectionStatus = "connected" | "disconnected" | "connecting" | "error";

// Active connection key for the current project
export const $activeConnection = atom<string | null>(null);

// Current project configuration
export const $projectConfig = atom<ProjectConfig | null>(null);

// Connection status for the active connection
export const $connectionStatus = atom<ConnectionStatus>("disconnected");

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
export function setActiveConnection(connKey: string) {
  $activeConnection.set(connKey);
  $connectionStatus.set("disconnected");
}

export function setProjectConfig(config: ProjectConfig | null) {
  $projectConfig.set(config);
}

export function setConnectionStatus(status: ConnectionStatus) {
  $connectionStatus.set(status);
}

export function toggleSidebar() {
  $sidebarVisible.set(!$sidebarVisible.get());
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

export async function switchConnection(connKey: string) {
  setActiveConnection(connKey);
  // Connection will be established in the component
}

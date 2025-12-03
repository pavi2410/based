import { atom } from "nanostores";
import { persistentAtom } from "@nanostores/persistent";

/**
 * Per-project state management using nanostores
 * Tracks active database, environment, and UI state for each project
 */

export interface RecentProject {
  path: string;
  name: string;
  lastOpened: string; // ISO timestamp
}

// Active database key for the current project
export const $activeDatabase = atom<string | null>(null);

// Active environment (dev, staging, prod, etc.)
export const $activeEnvironment = atom<string>("dev");

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
export function setActiveDatabase(dbKey: string) {
  $activeDatabase.set(dbKey);
}

export function setActiveEnvironment(env: string) {
  $activeEnvironment.set(env);
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

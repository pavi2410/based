import { invoke } from "@tauri-apps/api/core";
import type { SavedQuery, QuerySummary } from "../types/project";

/**
 * Query file store for managing saved queries (.query.toml files)
 * Handles listing, reading, writing, and deleting query files
 */

/**
 * List all saved queries in the project with summary info
 */
export async function listSavedQueries(
  projectPath: string,
): Promise<QuerySummary[]> {
  return await invoke("list_saved_queries", { projectPath });
}

/**
 * Get a saved query by filename
 */
export async function getSavedQuery(
  projectPath: string,
  filename: string,
): Promise<SavedQuery> {
  return await invoke("get_saved_query", { projectPath, filename });
}

/**
 * Save a query (create or update)
 */
export async function saveQuery(
  projectPath: string,
  filename: string,
  query: SavedQuery,
): Promise<void> {
  await invoke("save_query", { projectPath, filename, query });
}

/**
 * Delete a saved query
 */
export async function deleteSavedQuery(
  projectPath: string,
  filename: string,
): Promise<void> {
  await invoke("delete_saved_query", { projectPath, filename });
}

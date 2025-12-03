import { invoke } from "@tauri-apps/api/core";
import type { QueryFile, QueryMetadata } from "../types/project";

/**
 * Query file store for managing saved queries (.sqlx, .mongox files)
 * Handles listing, reading, writing, and deleting query files
 */

/**
 * List all query files in the project
 * Returns relative paths from .based/queries/
 */
export async function listQueryFiles(
  projectPath: string,
): Promise<string[]> {
  return await invoke("list_query_files", { projectPath });
}

/**
 * Read a query file and parse its YAML frontmatter
 */
export async function readQueryFile(
  projectPath: string,
  queryPath: string,
): Promise<QueryFile> {
  return await invoke("read_query_file", { projectPath, queryPath });
}

/**
 * Write a query file with YAML frontmatter
 */
export async function writeQueryFile(
  projectPath: string,
  queryPath: string,
  metadata: QueryMetadata,
  content: string,
): Promise<void> {
  await invoke("write_query_file", {
    projectPath,
    queryPath,
    metadata,
    content,
  });
}

/**
 * Delete a query file
 */
export async function deleteQueryFile(
  projectPath: string,
  queryPath: string,
): Promise<void> {
  await invoke("delete_query_file", { projectPath, queryPath });
}

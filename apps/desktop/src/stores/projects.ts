import { cmd } from "@/commands";
import type { ProjectConfig } from "../types/project";

/**
 * Project store for managing Based projects
 * Thin wrappers over the typed `cmd` surface.
 */

/**
 * Initialize a new Based project in the given directory.
 * Creates .based/ structure with config.toml, .env.example, .gitignore.
 */
export async function initializeProject(projectPath: string): Promise<void> {
  await cmd.initializeProject(projectPath);
}

/**
 * Read and parse project config from .based/config.toml
 */
export async function readProjectConfig(
  projectPath: string,
): Promise<ProjectConfig> {
  return await cmd.readProjectConfig(projectPath);
}

/**
 * Write project config to .based/config.toml
 */
export async function writeProjectConfig(
  projectPath: string,
  config: ProjectConfig,
): Promise<void> {
  await cmd.writeProjectConfig(projectPath, config);
}

import { invoke } from "@tauri-apps/api/core";
import type { ProjectConfig } from "../types/project";

/**
 * Project store for managing Based projects
 * Handles project discovery, initialization, and configuration
 */

/**
 * Initialize a new Based project in the given directory
 * Creates .based/ structure with config.toml, .env.example, .gitignore
 */
export async function initializeProject(projectPath: string): Promise<void> {
  await invoke("initialize_project", { projectPath });
}

/**
 * Read and parse project config from .based/config.toml
 */
export async function readProjectConfig(
  projectPath: string,
): Promise<ProjectConfig> {
  return await invoke("read_project_config", { projectPath });
}

/**
 * Write project config to .based/config.toml
 */
export async function writeProjectConfig(
  projectPath: string,
  config: ProjectConfig,
): Promise<void> {
  await invoke("write_project_config", { projectPath, config });
}

/**
 * Load environment variables from .based/.env file
 */
export async function loadEnvFile(
  projectPath: string,
): Promise<Record<string, string>> {
  return await invoke("load_env_file_command", { projectPath });
}

/**
 * Resolve connection config with environment overrides and variable interpolation
 * Merges base config + environment overrides, then resolves ${env:VAR} variables
 */
export async function resolveConnectionConfig(
  projectPath: string,
  dbKey: string,
  environment: string,
) {
  return await invoke("resolve_connection_config_command", {
    projectPath,
    dbKey,
    environment,
  });
}

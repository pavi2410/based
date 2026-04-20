/**
 * `useWorkspace()` — flattens the two-context (`ProjectContext` +
 * `ConnectionContext`) access pattern into a single hook.
 *
 * Motivation: every component in the workspace tree currently does
 *
 * ```ts
 * const proj = useProject();
 * const conn = useConnection();
 * const engine = conn.connectionConfig.engine;
 * // ...
 * ```
 *
 * That's three hook calls, one of which only returns a string, and
 * it makes the components harder to decompose because each
 * sub-component has to repeat the dance. Centralising the read-side
 * also means we can change how the state flows (e.g. swapping the
 * React Context for a store) without rewriting every consumer.
 *
 * `useWorkspace()` is intentionally _read-only_ — mutations like
 * "select a table" still go through the existing router navigate so
 * URL state stays the source of truth.
 *
 * Safe fallback: when rendered inside a detached child window where
 * `ProjectContext` isn't mounted (only `ConnectionContext` is set by
 * `DetachedTableViewer`), `projectConfig` is `null` and callers
 * should not assume it. The connection slice is always present
 * because `ConnectionContext` throws if missing.
 */
import { useContext } from "react";
import {
  ProjectContext,
  type ProjectContextValue,
} from "@/routes/project.$projectId";
import { useConnection } from "@/routes/project.$projectId/conn.$connKey";
import type { ConnectionConfig, Engine } from "@/types/project";

export interface WorkspaceSnapshot {
  /** Absolute filesystem path to the `.based/` project. */
  projectPath: string;
  /** Stable, URL-safe project identifier (base64 of `projectPath`). */
  projectId: string | null;
  /** Full parsed `config.toml` for the project, or `null` in detached windows. */
  projectConfig: ProjectContextValue["config"] | null;
  /** Force a re-read of `config.toml` from disk. `null` in detached windows. */
  reloadProjectConfig: (() => void) | null;

  /** The active connection's key as defined in `config.toml`. */
  connKey: string;
  /** The parsed `[connection.<connKey>]` block. */
  connectionConfig: ConnectionConfig;
  /** Convenience shortcut — the engine of the active connection. */
  engine: Engine;

  /** URL-selected table name, or `undefined` if no table is selected. */
  selectedTable: string | undefined;
  /** URL-selected schema (Postgres only), or `undefined`. */
  selectedSchema: string | undefined;

  /** Navigate the current window to a different table. No-op in detached windows. */
  selectTable: (name: string, schema?: string) => void;
}

export function useWorkspace(): WorkspaceSnapshot {
  // `ProjectContext` is read optionally because pop-out windows
  // deliberately don't mount it — they only carry `ConnectionContext`.
  const project = useContext(ProjectContext);
  const conn = useConnection();

  return {
    projectPath: conn.projectPath,
    projectId: project?.projectId ?? null,
    projectConfig: project?.config ?? null,
    reloadProjectConfig: project?.reloadConfig ?? null,

    connKey: conn.connKey,
    connectionConfig: conn.connectionConfig,
    engine: conn.connectionConfig.engine,

    selectedTable: conn.selectedTable,
    selectedSchema: conn.selectedSchema,

    selectTable: conn.onSelectTable,
  };
}

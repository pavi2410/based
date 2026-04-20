/**
 * Query history — a per-connection bounded log of executed queries.
 *
 * Stored in localStorage so history survives app restarts but stays
 * local to the device. Not written into `.based/` on purpose: query
 * history is inherently personal and noisy; mirroring it into git
 * would be user-hostile.
 *
 * Kept deliberately small (MAX_ENTRIES per connection) to keep the
 * localStorage payload tiny. Eviction is LRU by timestamp.
 */

import { persistentAtom } from "@nanostores/persistent";
import { atom } from "nanostores";

export interface HistoryEntry {
  /** UUID so history entries can be deleted individually. */
  id: string;
  projectPath: string;
  connKey: string;
  query: string;
  /** Epoch millis. */
  ranAt: number;
  /** How long the query took (ms). `null` when the query errored. */
  durationMs: number | null;
  /** Row count returned, or `null` if the query errored. */
  rowCount: number | null;
  /** Last error message (truncated) if the query failed. */
  error?: string;
}

const MAX_ENTRIES = 200;

export const $queryHistory = persistentAtom<HistoryEntry[]>(
  "based:query-history",
  [],
  {
    encode: JSON.stringify,
    decode: (s) => {
      try {
        return JSON.parse(s) as HistoryEntry[];
      } catch {
        return [];
      }
    },
  },
);

export function recordHistory(entry: Omit<HistoryEntry, "id">): void {
  const id = crypto.randomUUID?.() ?? String(Math.random());
  const next = [{ ...entry, id }, ...$queryHistory.get()].slice(0, MAX_ENTRIES);
  $queryHistory.set(next);
}

export function clearHistory(scope?: {
  projectPath: string;
  connKey: string;
}): void {
  if (!scope) {
    $queryHistory.set([]);
    return;
  }
  $queryHistory.set(
    $queryHistory
      .get()
      .filter(
        (e) =>
          e.projectPath !== scope.projectPath || e.connKey !== scope.connKey,
      ),
  );
}

export function historyForScope(scope: {
  projectPath: string;
  connKey: string;
}): HistoryEntry[] {
  return $queryHistory
    .get()
    .filter(
      (e) => e.projectPath === scope.projectPath && e.connKey === scope.connKey,
    );
}

/**
 * When the user picks a history entry (or any other "prefill this
 * query into a new draft editor" flow), we stash the SQL here and
 * navigate to the new-query route. QueryEditor reads this on mount
 * and clears it so the next new-query tab starts blank again.
 */
export const $pendingDraftQuery = atom<string | null>(null);

/**
 * Frontend mirror of currently-executing queries.
 *
 * We already have a Rust-side `QueryRegistry` that owns cancellation
 * handles. This store is a much lighter thing: just the set of query
 * tokens the UI has in-flight. It backs the global "N running"
 * indicator in the status bar and lets other surfaces (e.g. the
 * command palette's "Cancel all") discover running tokens without
 * a roundtrip.
 *
 * Consumers should call `markQueryStart(token)` when they kick off a
 * query and `markQueryEnd(token)` in a finally block. No persistence;
 * reloads drop the set on purpose.
 */
import { atom } from "nanostores";

export const $runningQueries = atom<string[]>([]);

export function markQueryStart(token: string): void {
  const current = $runningQueries.get();
  if (current.includes(token)) return;
  $runningQueries.set([...current, token]);
}

export function markQueryEnd(token: string): void {
  const current = $runningQueries.get();
  if (!current.includes(token)) return;
  $runningQueries.set(current.filter((t) => t !== token));
}

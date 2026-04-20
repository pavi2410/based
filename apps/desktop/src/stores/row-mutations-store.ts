/**
 * Session-scoped undo stack for row-level mutations.
 *
 * Each `UndoEntry` holds the *inverse* operation (the one we'd need to
 * apply to get back to the pre-edit state), not the original. The UI
 * layer calls `pushUndoEntry` after a successful mutation and `undo()`
 * runs the top entry.
 *
 * This intentionally lives outside React state so:
 *  - It survives re-renders / navigations within a project session.
 *  - Non-React callers (e.g. keyboard shortcut handlers) can read it.
 *
 * It is not persisted to disk; dev reloads reset the stack. Making
 * undo durable would need per-connection transactions / snapshots and
 * is out of scope for v1.
 */
import { atom } from "nanostores";

export interface UndoEntry {
  label: string;
  apply: () => Promise<void>;
}

/** Maximum entries retained; older entries are discarded. */
const MAX_ENTRIES = 50;

export const $undoStack = atom<UndoEntry[]>([]);

export function pushUndoEntry(entry: UndoEntry): void {
  const next = [...$undoStack.get(), entry];
  if (next.length > MAX_ENTRIES) {
    next.splice(0, next.length - MAX_ENTRIES);
  }
  $undoStack.set(next);
}

export function clearUndoStack(): void {
  $undoStack.set([]);
}

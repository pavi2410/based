/**
 * Tabs store — in-memory list of open tabs within a single connection.
 *
 * Kept deliberately simple for this first pass:
 *
 *  - Tabs are per-(project, conn) keyed by a stable string, so
 *    navigating between connections doesn't leak tab state from
 *    another connection.
 *  - The active tab is identified by its id; the route URL continues
 *    to be the source of truth for _what_ the active tab shows. The
 *    tab entries cache presentation bits (title, kind) so the bar can
 *    render without another IPC roundtrip.
 *  - No persistence yet. Phase 2.5 (workspace state) will back this
 *    onto `state/tabs.json`; storing to localStorage would silently
 *    break git-sharing of workspace state.
 *
 * Tab identity rules:
 *  - A "table" tab is keyed by `table:<schema>.<name>`. Opening the
 *    same table twice focuses the existing tab rather than creating a
 *    duplicate — this matches DataGrip / TablePlus / Beekeeper.
 *  - A saved-query tab is keyed by `query:<filename>`.
 *  - A new (unsaved) query tab gets a synthetic `query:new:<uuid>` id
 *    so drafts don't collide.
 */
import { atom, computed } from "nanostores";

export type TabKind = "table" | "query";

export interface OpenTab {
  id: string;
  kind: TabKind;
  title: string;
  /** project + connection this tab belongs to. */
  scope: TabScope;
  /** Table-kind metadata. */
  table?: { schema: string | null; name: string };
  /** Saved query filename. */
  queryFilename?: string;
  /** Set for unsaved draft query tabs. */
  isNewQuery?: boolean;
}

export interface TabScope {
  projectPath: string;
  connKey: string;
}

function scopeKey(s: TabScope): string {
  return `${s.projectPath}::${s.connKey}`;
}

/** Every open tab across every connection. */
export const $tabs = atom<OpenTab[]>([]);

/** Active tab id per (project, conn). */
export const $activeByScope = atom<Record<string, string | null>>({});

export const $tabsForActiveScope = (scope: TabScope) =>
  computed($tabs, (all) =>
    all.filter(
      (t) =>
        t.scope.projectPath === scope.projectPath &&
        t.scope.connKey === scope.connKey,
    ),
  );

export const $activeTabId = (scope: TabScope) =>
  computed($activeByScope, (m) => m[scopeKey(scope)] ?? null);

// ---------------------------------------------------------------------------
// Id builders. Exported so the route layer can synchronize URL → tab id
// without duplicating the string format.
// ---------------------------------------------------------------------------

export function tableTabId(
  schema: string | null | undefined,
  name: string,
): string {
  return `table:${schema ?? ""}.${name}`;
}

export function savedQueryTabId(filename: string): string {
  return `query:${filename}`;
}

export function newQueryTabId(): string {
  // Using crypto.randomUUID so ids are stable across remounts.
  const uuid =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : Math.random().toString(36).slice(2);
  return `query:new:${uuid}`;
}

// ---------------------------------------------------------------------------
// Mutations
// ---------------------------------------------------------------------------

/**
 * Ensure a tab exists for `tab.id` in the given scope. If a tab with
 * the same id is already open, update its title/metadata in place so
 * renames reflect without duplicating entries.
 */
export function upsertTab(tab: OpenTab): void {
  const current = $tabs.get();
  const existingIdx = current.findIndex(
    (t) =>
      t.id === tab.id &&
      t.scope.projectPath === tab.scope.projectPath &&
      t.scope.connKey === tab.scope.connKey,
  );
  if (existingIdx === -1) {
    $tabs.set([...current, tab]);
  } else {
    const copy = current.slice();
    copy[existingIdx] = { ...copy[existingIdx], ...tab };
    $tabs.set(copy);
  }
}

export function setActiveTab(scope: TabScope, id: string | null): void {
  const key = scopeKey(scope);
  $activeByScope.set({ ...$activeByScope.get(), [key]: id });
}

/**
 * Close a tab. Returns the id of the tab that should become active
 * next (the nearest neighbour) or `null` if there are no tabs left in
 * this scope. Callers are expected to navigate to the returned tab's
 * URL so the URL stays the source of truth.
 */
export function closeTab(scope: TabScope, id: string): string | null {
  const all = $tabs.get();
  const scoped = all.filter(
    (t) =>
      t.scope.projectPath === scope.projectPath &&
      t.scope.connKey === scope.connKey,
  );
  const idx = scoped.findIndex((t) => t.id === id);
  if (idx === -1) return $activeTabId(scope).get();

  const nextId = scoped[idx + 1]?.id ?? scoped[idx - 1]?.id ?? null;

  $tabs.set(
    all.filter(
      (t) =>
        !(
          t.id === id &&
          t.scope.projectPath === scope.projectPath &&
          t.scope.connKey === scope.connKey
        ),
    ),
  );

  const key = scopeKey(scope);
  const map = $activeByScope.get();
  if (map[key] === id) {
    $activeByScope.set({ ...map, [key]: nextId });
  }
  return nextId;
}

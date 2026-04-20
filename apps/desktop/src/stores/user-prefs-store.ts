/**
 * User preferences — device-scoped UI choices.
 *
 * Intentionally separate from project config (which lives in
 * `.based/config.toml` and is git-tracked). Prefs here are personal:
 * UI density, beginner/pro mode, last-opened layout, etc. They live
 * in localStorage so they survive restarts but don't follow the
 * project to another machine.
 *
 * Mode explainer:
 *   - "beginner": hides features that require SQL comfort (EXPLAIN,
 *     aggregate stage builder, raw mongo shell) and exposes guardrails
 *     like clearer confirm dialogs. Still fully functional for
 *     browse/filter/edit.
 *   - "pro": DataGrip-density, every feature visible.
 */
import { useStore } from "@nanostores/react";
import { persistentAtom } from "@nanostores/persistent";

export type UiMode = "beginner" | "pro";

export interface UserPrefs {
  mode: UiMode;
}

const DEFAULT_PREFS: UserPrefs = {
  mode: "pro",
};

export const $userPrefs = persistentAtom<UserPrefs>(
  "based:user-prefs",
  DEFAULT_PREFS,
  {
    encode: JSON.stringify,
    decode: (s) => {
      try {
        const parsed = JSON.parse(s) as Partial<UserPrefs>;
        return { ...DEFAULT_PREFS, ...parsed };
      } catch {
        return DEFAULT_PREFS;
      }
    },
  },
);

export function useUiMode(): UiMode {
  const prefs = useStore($userPrefs);
  return prefs.mode;
}

export function setUiMode(mode: UiMode): void {
  $userPrefs.set({ ...$userPrefs.get(), mode });
}

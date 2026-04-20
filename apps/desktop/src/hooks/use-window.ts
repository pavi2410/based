/**
 * Multi-window hook.
 *
 * Every window in the app is the same React bundle rendered with a
 * `?window=...` query param that encodes a `WindowKind`. This hook:
 *  - exposes the current window's kind (or `null` for the main window)
 *  - exposes typed openers / closers that call into the Rust
 *    `WindowManager` so we refocus instead of duplicate-spawning
 *
 * The hook is intentionally tiny: it is the only place the frontend
 * knows about OS windows. Every feature (result pop-out, tab detach,
 * settings) goes through it so we can swap the underlying
 * implementation (Tauri -> webview -> shared memory) without hunting
 * for call sites.
 */
import { useMemo } from "react";
import type { TabAddress, WindowKind } from "@/bindings";
import { cmd } from "@/commands";

export type { WindowKind } from "@/bindings";

/**
 * Parse the current window's `?window=<json>` query param.
 * Returns `null` for the main window (no `?window=` param) and logs a
 * warning if the payload fails to parse so we fail visibly rather than
 * silently rendering the wrong UI.
 */
function readCurrentWindow(): WindowKind | null {
  if (typeof window === "undefined") return null;
  const params = new URLSearchParams(window.location.search);
  const raw = params.get("window");
  if (!raw) return null;
  try {
    return JSON.parse(raw) as WindowKind;
  } catch (err) {
    console.warn("useWindow: failed to decode ?window= payload", err);
    return null;
  }
}

export function useWindow() {
  const current = useMemo(readCurrentWindow, []);

  return {
    /** The kind of the window this React tree is rendering in. */
    current,
    /** `true` when rendering inside the root "main" window. */
    isMain: current === null,

    /** Open (or focus if already open) a child window. */
    open: (kind: WindowKind) => cmd.openWindow(kind),

    /** Focus a previously-opened child window by its label. */
    focus: (label: string) => cmd.focusWindow(label),

    /** Close a previously-opened child window by its label. */
    close: (label: string) => cmd.closeWindow(label),

    // Convenience helpers so feature code doesn't build `WindowKind`
    // objects by hand.
    openTab: (address: TabAddress) => cmd.openWindow({ kind: "tab", address }),
    openResultViewer: (title: string) =>
      cmd.openWindow({ kind: "result_viewer", title }),
    openSettings: () => cmd.openWindow({ kind: "settings" }),
  };
}

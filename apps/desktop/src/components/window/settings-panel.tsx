/**
 * Settings panel rendered inside the detached Settings window.
 *
 * Scope is intentionally narrow: surface the handful of preferences
 * that already shape behaviour elsewhere (theme, beginner/pro mode),
 * plus a couple of knobs that previously required editing config.
 * When we later add per-project settings (default timeouts, auto-
 * formatting, etc.) they should go in a project-scoped panel; this
 * one is for the device.
 */

import { useStore } from "@nanostores/react";
import { useTheme } from "@/components/theme-provider";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  $userPrefs,
  setUiMode,
  type UiMode,
  useUiMode,
} from "@/stores/user-prefs-store";

export function SettingsPanel() {
  const { theme, setTheme } = useTheme();
  const uiMode = useUiMode();
  // Reading the whole store keeps this component rerender-safe once we
  // add more prefs (without having to remember to subscribe each new
  // field individually).
  useStore($userPrefs);

  return (
    <div className="flex flex-col h-screen w-screen bg-background">
      <div
        className="h-10 shrink-0 border-b bg-background/95 backdrop-blur-sm"
        data-tauri-drag-region
      />
      <div className="flex-1 overflow-auto">
        <div className="max-w-xl mx-auto px-8 py-8 space-y-8">
          <header className="space-y-1">
            <h1 className="text-lg font-semibold">Settings</h1>
            <p className="text-xs text-muted-foreground">
              Device-scoped preferences. Project settings live in{" "}
              <code className="bg-muted px-1 rounded">.based/config.toml</code>.
            </p>
          </header>

          <section className="space-y-3">
            <h2 className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
              Appearance
            </h2>
            <div className="flex items-center justify-between gap-4">
              <div>
                <Label>Theme</Label>
                <p className="text-xs text-muted-foreground">
                  Match your OS, or force a specific look.
                </p>
              </div>
              <Select
                value={theme}
                onValueChange={(v) => setTheme(v as typeof theme)}
              >
                <SelectTrigger className="w-[160px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="system">System</SelectItem>
                  <SelectItem value="light">Light</SelectItem>
                  <SelectItem value="dark">Dark</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </section>

          <section className="space-y-3">
            <h2 className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
              Interface
            </h2>
            <div className="flex items-center justify-between gap-4">
              <div>
                <Label>Mode</Label>
                <p className="text-xs text-muted-foreground">
                  Beginner hides advanced actions (EXPLAIN, aggregate builder).
                  Pro shows everything.
                </p>
              </div>
              <Select
                value={uiMode}
                onValueChange={(v) => setUiMode(v as UiMode)}
              >
                <SelectTrigger className="w-[160px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="beginner">Beginner</SelectItem>
                  <SelectItem value="pro">Pro</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}

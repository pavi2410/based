/**
 * Command palette — the app-wide `Cmd/Ctrl+K` action launcher.
 *
 * Goals (v1):
 *   - Surface the handful of actions that are *always* useful: new
 *     query, theme switch, disconnect, open settings, close current
 *     tab. Feature-specific actions can register later.
 *   - Keep the implementation a single file so any feature can drop
 *     in a command by pushing onto an array. Don't build a registry
 *     abstraction until there are >2 consumers.
 *
 * The palette is mounted once at the top of the app; it listens for
 * `Cmd/Ctrl+K` on the document and opens itself. Individual commands
 * only need to know their own handlers.
 */
import { useNavigate } from "@tanstack/react-router";
import {
  FilePlusIcon,
  MoonIcon,
  SettingsIcon,
  SunIcon,
  XCircleIcon,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTheme } from "@/components/theme-provider";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import { useWindow } from "@/hooks/use-window";

export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const { setTheme } = useTheme();
  const navigate = useNavigate();
  const { openSettings, isMain } = useWindow();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((o) => !o);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  const run = useCallback((fn: () => void | Promise<void>) => {
    setOpen(false);
    void Promise.resolve(fn()).catch((err) => {
      console.error("command failed", err);
    });
  }, []);

  if (!isMain) return null; // Palette is main-window-only for now.

  return (
    <CommandDialog open={open} onOpenChange={setOpen}>
      <CommandInput placeholder="Type a command..." />
      <CommandList>
        <CommandEmpty>No results.</CommandEmpty>
        <CommandGroup heading="Actions">
          <CommandItem
            onSelect={() =>
              run(() =>
                navigate({
                  to: ".",
                  search: (s) => ({ ...s, newQuery: true }),
                }),
              )
            }
          >
            <FilePlusIcon className="size-4" />
            New query
          </CommandItem>
          <CommandItem
            onSelect={() =>
              run(() =>
                navigate({
                  to: ".",
                  search: (s) => ({
                    ...s,
                    newQuery: undefined,
                    query: undefined,
                    table: undefined,
                  }),
                }),
              )
            }
          >
            <XCircleIcon className="size-4" />
            Close current tab
          </CommandItem>
          <CommandItem
            onSelect={() =>
              run(async () => {
                await openSettings();
              })
            }
          >
            <SettingsIcon className="size-4" />
            Open settings
          </CommandItem>
        </CommandGroup>
        <CommandGroup heading="Theme">
          <CommandItem onSelect={() => run(() => setTheme("light"))}>
            <SunIcon className="size-4" />
            Light
          </CommandItem>
          <CommandItem onSelect={() => run(() => setTheme("dark"))}>
            <MoonIcon className="size-4" />
            Dark
          </CommandItem>
          <CommandItem onSelect={() => run(() => setTheme("system"))}>
            <SunIcon className="size-4 opacity-50" />
            System
          </CommandItem>
        </CommandGroup>
      </CommandList>
    </CommandDialog>
  );
}

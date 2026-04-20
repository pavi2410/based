/**
 * Root component for child windows.
 *
 * Main-window rendering (the full router / project workspace) is
 * unaffected. Every other OS window (detached tab, result pop-out,
 * settings) renders a tiny shell that reads the current `WindowKind`
 * from `useWindow()` and dispatches to the appropriate panel.
 */
import { useWindow } from "@/hooks/use-window";
import { DetachedTableViewer } from "@/components/window/detached-table-viewer";
import { SettingsPanel } from "@/components/window/settings-panel";

function PlaceholderPanel({
  title,
  children,
}: {
  title: string;
  children?: React.ReactNode;
}) {
  return (
    <div
      className="flex flex-col h-screen w-screen"
      data-tauri-drag-region
      role="main"
    >
      <div
        className="h-8 shrink-0 border-b bg-muted/30"
        data-tauri-drag-region
      />
      <div className="flex-1 overflow-auto p-6 space-y-2">
        <h1 className="text-lg font-semibold">{title}</h1>
        <div className="text-sm text-muted-foreground">{children}</div>
      </div>
    </div>
  );
}

export function WindowShell() {
  const { current } = useWindow();
  if (!current) {
    // Should never render in the main window; kept as a safety net.
    return <PlaceholderPanel title="based">Main window.</PlaceholderPanel>;
  }

  switch (current.kind) {
    case "tab": {
      const { address } = current;
      if (address.kind === "table") {
        return <DetachedTableViewer address={address} />;
      }
      const summary =
        address.kind === "query"
          ? `Query tab ${address.id}`
          : `Inspector ${address.name}`;
      return (
        <PlaceholderPanel title={summary}>
          Detached {address.kind} tab — rendering lands with the Phase 2 tabbed
          workspace.
        </PlaceholderPanel>
      );
    }
    case "result_viewer":
      return (
        <PlaceholderPanel title={current.title}>
          Pop-out result viewer — rendering TBD (see Phase 1 pop-out work).
        </PlaceholderPanel>
      );
    case "settings":
      return <SettingsPanel />;
  }
}

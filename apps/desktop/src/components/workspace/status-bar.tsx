/**
 * Thin bottom-of-window status strip for the connection workspace.
 *
 * The top-bar already signals connection health, but it competes with
 * the project switcher and action buttons for attention. A dedicated
 * status bar gives cheap ambient visibility into:
 *   - which engine/connection you're in (engine icon + label),
 *   - server handshake time + wall-clock connected-at,
 *   - whether a query is currently running (via QueryRegistry store),
 *   - current beginner/pro mode, since it affects what actions are
 *     visible on the page.
 *
 * We keep it sub-20px tall so it doesn't eat into the data grid.
 */
import { useStore } from "@nanostores/react";
import {
  CircleIcon,
  GraduationCapIcon,
  Loader2Icon,
  WrenchIcon,
} from "lucide-react";
import { $connection } from "@/stores/project-state";
import { $runningQueries } from "@/stores/query-registry-store";
import { useUiMode } from "@/stores/user-prefs-store";
import type { ConnectionConfig, Engine } from "@/types/project";
import DeviconMongodb from "~icons/devicon/mongodb";
import DeviconPostgresql from "~icons/devicon/postgresql";
import DeviconSqlite from "~icons/devicon/sqlite";

function engineIcon(engine: Engine) {
  switch (engine) {
    case "sqlite":
      return <DeviconSqlite className="size-3" />;
    case "mongodb":
      return <DeviconMongodb className="size-3" />;
    case "postgres":
      return <DeviconPostgresql className="size-3" />;
  }
}

function formatMs(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

export function StatusBar({
  connKey,
  connectionConfig,
}: {
  connKey: string;
  connectionConfig: ConnectionConfig;
}) {
  const connection = useStore($connection);
  const running = useStore($runningQueries);
  const uiMode = useUiMode();

  const stats = connection.stats;
  const runCount = running.length;

  const statusColor =
    connection.status === "connected"
      ? "text-emerald-500 fill-emerald-500"
      : connection.status === "connecting"
        ? "text-amber-500 fill-amber-500 animate-pulse"
        : connection.status === "error"
          ? "text-red-500 fill-red-500"
          : "text-muted-foreground/50 fill-muted-foreground/50";

  return (
    <div className="h-5 shrink-0 border-t bg-background/95 flex items-center px-2 gap-3 text-[11px] text-muted-foreground select-none">
      <div className="flex items-center gap-1.5">
        <CircleIcon className={`size-1.5 ${statusColor}`} />
        {engineIcon(connectionConfig.engine)}
        <span className="text-foreground/80 font-medium">
          {connectionConfig.label || connKey}
        </span>
      </div>

      {stats && connection.status === "connected" ? (
        <span className="tabular-nums">
          {formatMs(stats.connectionTimeMs)} ·{" "}
          {new Date(stats.connectedAt).toLocaleTimeString()}
        </span>
      ) : null}

      <div className="flex-1" />

      {runCount > 0 ? (
        <span className="flex items-center gap-1 text-amber-500">
          <Loader2Icon className="size-3 animate-spin" />
          {runCount} running
        </span>
      ) : null}

      <span className="flex items-center gap-1">
        {uiMode === "beginner" ? (
          <>
            <GraduationCapIcon className="size-3" />
            Beginner
          </>
        ) : (
          <>
            <WrenchIcon className="size-3" />
            Pro
          </>
        )}
      </span>
    </div>
  );
}

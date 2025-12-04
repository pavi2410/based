import { CircleDotIcon } from "lucide-react";
import DeviconSqlite from "~icons/devicon/sqlite";
import DeviconMongodb from "~icons/devicon/mongodb";
import DeviconPostgresql from "~icons/devicon/postgresql";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { ConnectionConfig } from "@/types/project";

interface ConnectionSelectorProps {
  connections: Record<string, ConnectionConfig>;
  connKey: string | null;
  onConnectionChange: (connKey: string) => void;
  /** Compact mode for status bar */
  compact?: boolean;
}

function getEngineIcon(engine: string) {
  switch (engine) {
    case "sqlite":
      return <DeviconSqlite className="size-4" />;
    case "mongodb":
      return <DeviconMongodb className="size-4" />;
    case "postgres":
      return <DeviconPostgresql className="size-4" />;
    default:
      return <CircleDotIcon className="size-4" />;
  }
}

export function ConnectionSelector({
  connections,
  connKey,
  onConnectionChange,
  compact = false,
}: ConnectionSelectorProps) {
  // Group connections by group field, then by engine if no group
  const enabledConnections = Object.entries(connections)
    .filter(([, conn]) => !conn.disabled)
    .map(([key, conn]) => ({ key, ...conn }));

  const groupedConnections = Object.groupBy(
    enabledConnections,
    (conn) => conn.group || conn.engine,
  );

  // Sort group names and connections within each group
  const sortedGroups = Object.entries(groupedConnections)
    .toSorted(([a], [b]) => a.localeCompare(b))
    .map(([groupKey, conns]) => ([
      groupKey,
      conns!.toSorted((a, b) => (a.order || 0) - (b.order || 0) || a.key.localeCompare(b.key)),
    ] as const));

  const activeConnConfig = connKey ? connections[connKey] : null;

  return (
    <Select value={connKey || undefined} onValueChange={onConnectionChange}>
      <SelectTrigger className={compact ? "h-6 text-xs gap-1 border-none bg-transparent [--spacing:3px]" : "w-[280px]"}>
        <SelectValue placeholder="Select connection">
          {activeConnConfig && (
            <div className="flex items-center gap-1.5">
              {getEngineIcon(activeConnConfig.engine)}
              <span>{activeConnConfig.label || connKey}</span>
            </div>
          )}
        </SelectValue>
      </SelectTrigger>
      <SelectContent>
        {sortedGroups.map(([groupKey, conns]) => (
          <SelectGroup key={groupKey}>
            <SelectLabel className="flex items-center gap-2 text-xs capitalize">
              {groupKey}
            </SelectLabel>
            {conns.map((conn) => (
              <SelectItem key={conn.key} value={conn.key}>
                <div className="flex items-center gap-2 w-full">
                  {getEngineIcon(conn.engine)}
                  <span className="flex-1">{conn.label || conn.key}</span>
                </div>
              </SelectItem>
            ))}
          </SelectGroup>
        ))}
      </SelectContent>
    </Select>
  );
}

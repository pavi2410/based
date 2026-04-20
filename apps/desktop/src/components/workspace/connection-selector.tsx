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
  connections: Partial<Record<string, ConnectionConfig>>;
  connKey: string | null;
  onConnectionChange: (connKey: string) => void;
  /** Compact mode for status bar */
  compact?: boolean;
}

function getEngineIcon(engine: string, compact = false) {
  const size = compact ? "size-3" : "size-4";
  switch (engine) {
    case "sqlite":
      return <DeviconSqlite className={size} />;
    case "mongodb":
      return <DeviconMongodb className={size} />;
    case "postgres":
      return <DeviconPostgresql className={size} />;
    default:
      return <CircleDotIcon className={size} />;
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
    .filter(
      (entry): entry is [string, ConnectionConfig] =>
        !!entry[1] && !entry[1].disabled,
    )
    .map(([key, conn]) => ({ key, ...conn }));

  const groupedConnections = Object.groupBy(
    enabledConnections,
    (conn) => conn.group || conn.engine,
  );

  // Sort group names and connections within each group
  const sortedGroups = Object.entries(groupedConnections)
    .toSorted(([a], [b]) => a.localeCompare(b))
    .map(
      ([groupKey, conns]) =>
        [
          groupKey,
          conns!.toSorted(
            (a, b) =>
              (a.order || 0) - (b.order || 0) || a.key.localeCompare(b.key),
          ),
        ] as const,
    );

  const activeConnConfig = connKey ? connections[connKey] : null;

  return (
    <Select value={connKey || undefined} onValueChange={onConnectionChange}>
      <SelectTrigger
        size="sm"
        className={
          compact
            ? "h-auto! py-1 px-2 text-xs gap-1.5 border-none bg-transparent shadow-none hover:bg-muted/50 [&_svg:last-child]:size-3"
            : "w-[280px]"
        }
      >
        <SelectValue placeholder="Select connection">
          {activeConnConfig && (
            <div className="flex items-center gap-1">
              {getEngineIcon(activeConnConfig.engine, compact)}
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

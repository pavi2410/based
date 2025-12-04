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
  activeConnection: string | null;
  onConnectionChange: (connKey: string) => void;
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
  activeConnection,
  onConnectionChange,
}: ConnectionSelectorProps) {
  // Group connections by group field, then by engine if no group
  const groupedConnections = Object.entries(connections).reduce(
    (acc, [key, conn]) => {
      // Skip disabled connections
      if (conn.disabled) {
        return acc;
      }

      const groupKey = conn.group || conn.engine;
      if (!acc[groupKey]) {
        acc[groupKey] = [];
      }
      acc[groupKey].push({ key, ...conn });
      return acc;
    },
    {} as Record<string, Array<{ key: string } & ConnectionConfig>>,
  );

  // Sort connections within each group by order field
  Object.values(groupedConnections).forEach((group) => {
    group.sort((a, b) => (a.order || 0) - (b.order || 0));
  });

  const activeConnConfig = activeConnection ? connections[activeConnection] : null;

  return (
    <Select value={activeConnection || undefined} onValueChange={onConnectionChange}>
      <SelectTrigger className="w-[280px]">
        <SelectValue placeholder="Select connection">
          {activeConnConfig && (
            <div className="flex items-center gap-2">
              {getEngineIcon(activeConnConfig.engine)}
              <span>{activeConnConfig.label || activeConnection}</span>
            </div>
          )}
        </SelectValue>
      </SelectTrigger>
      <SelectContent>
        {Object.entries(groupedConnections).map(([groupKey, conns]) => (
          <SelectGroup key={groupKey}>
            <SelectLabel className="flex items-center gap-2 text-xs">
              <CircleDotIcon className="size-3" />
              {groupKey.charAt(0).toUpperCase() + groupKey.slice(1)}
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

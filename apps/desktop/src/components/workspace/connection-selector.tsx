import { DatabaseIcon, CheckIcon, CircleDotIcon } from "lucide-react";
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
import { Badge } from "@/components/ui/badge";

interface ConnectionSelectorProps {
  connections: Record<string, ConnectionConfig>;
  activeConnection: string | null;
  onConnectionChange: (connKey: string) => void;
}

function getEngineLabel(engine: string): string {
  switch (engine) {
    case "sqlite":
      return "SQLite";
    case "mongodb":
      return "MongoDB";
    case "postgres":
      return "PostgreSQL";
    default:
      return engine;
  }
}

function getEngineColor(engine: string): string {
  switch (engine) {
    case "sqlite":
      return "text-blue-500";
    case "mongodb":
      return "text-green-500";
    case "postgres":
      return "text-purple-500";
    default:
      return "text-gray-500";
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
      <SelectTrigger className="w-[250px]">
        <div className="flex items-center gap-2">
          <DatabaseIcon className="size-4" />
          <SelectValue placeholder="Select connection">
            {activeConnConfig && (
              <div className="flex items-center gap-2">
                <span>{activeConnConfig.label || activeConnection}</span>
                <Badge variant="secondary" className="text-xs">
                  {getEngineLabel(activeConnConfig.engine)}
                </Badge>
              </div>
            )}
          </SelectValue>
        </div>
      </SelectTrigger>
      <SelectContent>
        {Object.entries(groupedConnections).map(([groupKey, conns]) => (
          <SelectGroup key={groupKey}>
            <SelectLabel className="flex items-center gap-2">
              <CircleDotIcon className={`size-3 ${getEngineColor(conns[0].engine)}`} />
              {groupKey.charAt(0).toUpperCase() + groupKey.slice(1)}
            </SelectLabel>
            {conns.map((conn) => (
              <SelectItem key={conn.key} value={conn.key}>
                <div className="flex items-center gap-2">
                  {activeConnection === conn.key && (
                    <CheckIcon className="size-3 text-primary" />
                  )}
                  <span>{conn.label || conn.key}</span>
                  {conn.color && (
                    <div
                      className="size-2 rounded-full"
                      style={{ backgroundColor: conn.color }}
                    />
                  )}
                </div>
              </SelectItem>
            ))}
          </SelectGroup>
        ))}
      </SelectContent>
    </Select>
  );
}

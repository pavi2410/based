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
import type { DatabaseConfig } from "@/types/project";
import { Badge } from "@/components/ui/badge";

interface DatabaseSelectorProps {
  databases: Record<string, DatabaseConfig>;
  activeDatabase: string | null;
  onDatabaseChange: (dbKey: string) => void;
}

function getDatabaseTypeLabel(type: string): string {
  switch (type) {
    case "sqlite":
      return "SQLite";
    case "mongodb":
      return "MongoDB";
    case "postgresql":
      return "PostgreSQL";
    default:
      return type;
  }
}

function getDatabaseTypeColor(type: string): string {
  switch (type) {
    case "sqlite":
      return "text-blue-500";
    case "mongodb":
      return "text-green-500";
    case "postgresql":
      return "text-purple-500";
    default:
      return "text-gray-500";
  }
}

export function DatabaseSelector({
  databases,
  activeDatabase,
  onDatabaseChange,
}: DatabaseSelectorProps) {
  // Group databases by type
  const groupedDatabases = Object.entries(databases).reduce(
    (acc, [key, db]) => {
      if (!acc[db.type]) {
        acc[db.type] = [];
      }
      acc[db.type].push({ key, ...db });
      return acc;
    },
    {} as Record<string, Array<{ key: string } & DatabaseConfig>>,
  );

  const activeDbConfig = activeDatabase ? databases[activeDatabase] : null;

  return (
    <Select value={activeDatabase || undefined} onValueChange={onDatabaseChange}>
      <SelectTrigger className="w-[250px]">
        <div className="flex items-center gap-2">
          <DatabaseIcon className="size-4" />
          <SelectValue placeholder="Select database">
            {activeDbConfig && (
              <div className="flex items-center gap-2">
                <span>{activeDbConfig.name}</span>
                <Badge variant="secondary" className="text-xs">
                  {getDatabaseTypeLabel(activeDbConfig.type)}
                </Badge>
              </div>
            )}
          </SelectValue>
        </div>
      </SelectTrigger>
      <SelectContent>
        {Object.entries(groupedDatabases).map(([type, dbs]) => (
          <SelectGroup key={type}>
            <SelectLabel className="flex items-center gap-2">
              <CircleDotIcon className={`size-3 ${getDatabaseTypeColor(type)}`} />
              {getDatabaseTypeLabel(type)}
            </SelectLabel>
            {dbs.map((db) => (
              <SelectItem key={db.key} value={db.key}>
                <div className="flex items-center gap-2">
                  {activeDatabase === db.key && (
                    <CheckIcon className="size-3 text-primary" />
                  )}
                  <span>{db.name}</span>
                </div>
              </SelectItem>
            ))}
          </SelectGroup>
        ))}
      </SelectContent>
    </Select>
  );
}

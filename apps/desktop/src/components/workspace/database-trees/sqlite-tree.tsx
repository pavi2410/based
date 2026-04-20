import { useQuery } from "@tanstack/react-query";
import {
  ChevronRightIcon,
  ListOrderedIcon,
  RefreshCcwIcon,
  Table2Icon,
  TableIcon,
} from "lucide-react";
import { useState } from "react";
import { cmd } from "@/commands";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { queryKeys } from "@/lib/query-keys";
import type { ConnectionConfig } from "@/types/project";

interface SQLiteDatabaseTreeProps {
  connKey: string;
  connConfig: ConnectionConfig;
  projectPath: string;
  onSelectTable?: (tableName: string, schema?: string) => void;
  selectedTable?: string;
}

export function SQLiteDatabaseTree({
  connKey,
  connConfig,
  projectPath,
  onSelectTable,
  selectedTable,
}: SQLiteDatabaseTreeProps) {
  return (
    <div className="py-1 space-y-0.5">
      <SQLiteObjectGroup
        connKey={connKey}
        connConfig={connConfig}
        projectPath={projectPath}
        type="table"
        label="Tables"
        icon={<TableIcon className="size-3.5" />}
        onSelectTable={onSelectTable}
        selectedTable={selectedTable}
        defaultOpen
      />
      <SQLiteObjectGroup
        connKey={connKey}
        connConfig={connConfig}
        projectPath={projectPath}
        type="view"
        label="Views"
        icon={<Table2Icon className="size-3.5" />}
        onSelectTable={onSelectTable}
        selectedTable={selectedTable}
      />
      <SQLiteObjectGroup
        connKey={connKey}
        connConfig={connConfig}
        projectPath={projectPath}
        type="index"
        label="Indexes"
        icon={<ListOrderedIcon className="size-3.5" />}
      />
      <SQLiteObjectGroup
        connKey={connKey}
        connConfig={connConfig}
        projectPath={projectPath}
        type="trigger"
        label="Triggers"
        icon={<RefreshCcwIcon className="size-3.5" />}
      />
    </div>
  );
}

interface SQLiteObjectGroupProps {
  connKey: string;
  connConfig: ConnectionConfig;
  projectPath: string;
  type: string;
  label: string;
  icon: React.ReactNode;
  onSelectTable?: (tableName: string, schema?: string) => void;
  selectedTable?: string;
  defaultOpen?: boolean;
}

function SQLiteObjectGroup({
  connKey,
  projectPath,
  type,
  label,
  icon,
  onSelectTable,
  selectedTable,
  defaultOpen = false,
}: SQLiteObjectGroupProps) {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  const objectQuery = useQuery({
    queryKey: queryKeys.conn.sqliteObjects(projectPath, connKey, type),
    queryFn: async () => {
      return await cmd.getSqliteObjects(projectPath, connKey, type);
    },
    enabled: isOpen, // Only fetch when expanded
  });

  const handleObjectClick = (name: string) => {
    onSelectTable?.(name);
  };

  return (
    <Collapsible open={isOpen} onOpenChange={setIsOpen}>
      <CollapsibleTrigger
        render={
          <button
            type="button"
            className="w-full flex items-center gap-1.5 px-2 h-7 text-xs hover:bg-muted/50 transition-colors"
          >
            <ChevronRightIcon
              className={`size-3 text-muted-foreground transition-transform ${isOpen ? "rotate-90" : ""}`}
            />
            <span className="text-muted-foreground">{icon}</span>
            <span className="flex-1 text-left font-medium">{label}</span>
            <span className="text-[10px] text-muted-foreground tabular-nums">
              {objectQuery.isSuccess ? objectQuery.data.length : "–"}
            </span>
          </button>
        }
      />
      <CollapsibleContent className="ml-4 border-l border-border/50">
        {objectQuery.isLoading && (
          <div className="text-[11px] text-muted-foreground px-3 py-1.5">
            Loading...
          </div>
        )}
        {objectQuery.isError && (
          <div className="text-[11px] text-destructive px-3 py-1.5">
            Failed to load
          </div>
        )}
        {objectQuery.isSuccess && objectQuery.data.length === 0 && (
          <div className="text-[11px] text-muted-foreground px-3 py-1.5 italic">
            None
          </div>
        )}
        {objectQuery.isSuccess &&
          objectQuery.data.map((obj) => {
            const isSelected = selectedTable === obj.name;
            return (
              <button
                type="button"
                key={obj.name}
                className={`w-full text-left h-6 px-3 text-[11px] truncate transition-colors ${
                  isSelected
                    ? "bg-primary/10 text-primary font-medium"
                    : "text-foreground/80 hover:bg-muted/50"
                }`}
                title={obj.name}
                onClick={() => handleObjectClick(obj.name)}
              >
                {obj.name}
              </button>
            );
          })}
      </CollapsibleContent>
    </Collapsible>
  );
}

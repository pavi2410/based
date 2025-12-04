import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import { useStore } from "@nanostores/react";
import { TableIcon, Table2Icon, ListOrderedIcon, RefreshCcwIcon, ChevronRightIcon } from "lucide-react";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { ConnectionConfig } from "@/types/project";
import { selectObject, $selectedObject } from "@/stores/project-state";

interface SQLiteDatabaseTreeProps {
  connKey: string;
  connConfig: ConnectionConfig;
  projectPath: string;
}

interface SQLiteObject {
  name: string;
}

export function SQLiteDatabaseTree({
  connKey,
  connConfig,
  projectPath,
}: SQLiteDatabaseTreeProps) {
  return (
    <div className="p-2 space-y-2">
      <SQLiteObjectGroup
        connKey={connKey}
        connConfig={connConfig}
        projectPath={projectPath}
        type="table"
        label="Tables"
        icon={<TableIcon className="size-4" />}
      />
      <SQLiteObjectGroup
        connKey={connKey}
        connConfig={connConfig}
        projectPath={projectPath}
        type="view"
        label="Views"
        icon={<Table2Icon className="size-4" />}
      />
      <SQLiteObjectGroup
        connKey={connKey}
        connConfig={connConfig}
        projectPath={projectPath}
        type="index"
        label="Indexes"
        icon={<ListOrderedIcon className="size-4" />}
      />
      <SQLiteObjectGroup
        connKey={connKey}
        connConfig={connConfig}
        projectPath={projectPath}
        type="trigger"
        label="Triggers"
        icon={<RefreshCcwIcon className="size-4" />}
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
}

function SQLiteObjectGroup({
  connKey,
  projectPath,
  type,
  label,
  icon,
}: SQLiteObjectGroupProps) {
  const [isOpen, setIsOpen] = useState(false);
  const selectedObject = useStore($selectedObject);

  const objectQuery = useQuery({
    queryKey: ["project-db-objects", projectPath, connKey, type],
    queryFn: async () => {
      // Call Tauri command to get objects for this connection
      const objects = await invoke<SQLiteObject[]>("get_sqlite_objects", {
        projectPath,
        connKey,
        objectType: type,
      });
      return objects;
    },
    enabled: isOpen, // Only fetch when expanded
  });

  const handleObjectClick = (name: string) => {
    selectObject({
      type: type === "table" ? "table" : "view",
      name,
    });
  };

  return (
    <Collapsible open={isOpen} onOpenChange={setIsOpen}>
      <CollapsibleTrigger asChild>
        <Button
          variant="ghost"
          className="w-full justify-start gap-2 px-2 h-8"
        >
          <ChevronRightIcon
            className={`size-4 transition-transform ${isOpen ? "rotate-90" : ""}`}
          />
          {icon}
          <span className="flex-1 text-left text-sm">{label}</span>
          <Badge variant="outline" className="text-xs">
            {objectQuery.isSuccess ? objectQuery.data.length : 0}
          </Badge>
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent className="ml-6 mt-1 space-y-1">
        {objectQuery.isLoading && (
          <div className="text-xs text-muted-foreground px-2 py-1">
            Loading...
          </div>
        )}
        {objectQuery.isError && (
          <div className="text-xs text-destructive px-2 py-1">
            Failed to load {label.toLowerCase()}
          </div>
        )}
        {objectQuery.isSuccess && objectQuery.data.length === 0 && (
          <div className="text-xs text-muted-foreground px-2 py-1">
            No {label.toLowerCase()} found
          </div>
        )}
        {objectQuery.isSuccess &&
          objectQuery.data.map((obj) => {
            const isSelected = selectedObject?.name === obj.name && !selectedObject?.schema;
            return (
              <Button
                key={obj.name}
                variant={isSelected ? "secondary" : "ghost"}
                className="w-full justify-start h-7 px-2 text-xs font-normal"
                title={obj.name}
                onClick={() => handleObjectClick(obj.name)}
              >
                {obj.name}
              </Button>
            );
          })}
      </CollapsibleContent>
    </Collapsible>
  );
}

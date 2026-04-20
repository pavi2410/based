import { useState } from "react";
import { cmd } from "@/commands";
import { useQuery } from "@tanstack/react-query";
import { DatabaseIcon, ChevronRightIcon } from "lucide-react";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import type { ConnectionConfig } from "@/types/project";

interface MongoDBDatabaseTreeProps {
  connKey: string;
  connConfig: ConnectionConfig;
  projectPath: string;
  onSelectTable?: (tableName: string, schema?: string) => void;
  selectedTable?: string;
}

export function MongoDBDatabaseTree({
  connKey,
  projectPath,
  onSelectTable,
  selectedTable,
}: MongoDBDatabaseTreeProps) {
  const [isOpen, setIsOpen] = useState(true);

  const collectionsQuery = useQuery({
    queryKey: ["project-db-collections", projectPath, connKey],
    queryFn: async () => {
      return await cmd.getMongodbCollections(projectPath, connKey);
    },
    enabled: isOpen,
  });

  const handleCollectionClick = (name: string) => {
    onSelectTable?.(name);
  };

  return (
    <div className="py-1 space-y-0.5">
      <Collapsible open={isOpen} onOpenChange={setIsOpen}>
        <CollapsibleTrigger asChild>
          <button className="w-full flex items-center gap-1.5 px-2 h-7 text-xs hover:bg-muted/50 transition-colors">
            <ChevronRightIcon
              className={`size-3 text-muted-foreground transition-transform ${isOpen ? "rotate-90" : ""}`}
            />
            <DatabaseIcon className="size-3.5 text-muted-foreground" />
            <span className="flex-1 text-left font-medium">Collections</span>
            <span className="text-[10px] text-muted-foreground tabular-nums">
              {collectionsQuery.isSuccess ? collectionsQuery.data.length : "–"}
            </span>
          </button>
        </CollapsibleTrigger>
        <CollapsibleContent className="ml-4 border-l border-border/50">
          {collectionsQuery.isLoading && (
            <div className="text-[11px] text-muted-foreground px-3 py-1.5">
              Loading...
            </div>
          )}
          {collectionsQuery.isError && (
            <div className="text-[11px] text-destructive px-3 py-1.5">
              Failed to load
            </div>
          )}
          {collectionsQuery.isSuccess && collectionsQuery.data.length === 0 && (
            <div className="text-[11px] text-muted-foreground px-3 py-1.5 italic">
              None
            </div>
          )}
          {collectionsQuery.isSuccess &&
            collectionsQuery.data.map((collection) => {
              const isSelected = selectedTable === collection.name;
              return (
                <button
                  key={collection.name}
                  className={`w-full text-left h-6 px-3 text-[11px] truncate transition-colors ${
                    isSelected
                      ? "bg-primary/10 text-primary font-medium"
                      : "text-foreground/80 hover:bg-muted/50"
                  }`}
                  title={collection.name}
                  onClick={() => handleCollectionClick(collection.name)}
                >
                  {collection.name}
                </button>
              );
            })}
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}

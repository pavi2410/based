import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import { DatabaseIcon, ChevronRightIcon } from "lucide-react";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { DatabaseConfig } from "@/types/project";

interface MongoDBDatabaseTreeProps {
  dbKey: string;
  dbConfig: DatabaseConfig;
  projectPath: string;
  environment: string;
}

interface MongoCollection {
  name: string;
}

export function MongoDBDatabaseTree({
  dbKey,
  projectPath,
  environment,
}: MongoDBDatabaseTreeProps) {
  const [isOpen, setIsOpen] = useState(true);

  const collectionsQuery = useQuery({
    queryKey: ["project-db-collections", projectPath, dbKey, environment],
    queryFn: async () => {
      const collections = await invoke<MongoCollection[]>("get_mongodb_collections", {
        projectPath,
        dbKey,
        environment,
      });
      return collections;
    },
    enabled: isOpen,
  });

  return (
    <div className="p-2">
      <Collapsible open={isOpen} onOpenChange={setIsOpen}>
        <CollapsibleTrigger asChild>
          <Button
            variant="ghost"
            className="w-full justify-start gap-2 px-2 h-8"
          >
            <ChevronRightIcon
              className={`size-4 transition-transform ${isOpen ? "rotate-90" : ""}`}
            />
            <DatabaseIcon className="size-4" />
            <span className="flex-1 text-left text-sm">Collections</span>
            <Badge variant="outline" className="text-xs">
              {collectionsQuery.isSuccess ? collectionsQuery.data.length : 0}
            </Badge>
          </Button>
        </CollapsibleTrigger>
        <CollapsibleContent className="ml-6 mt-1 space-y-1">
          {collectionsQuery.isLoading && (
            <div className="text-xs text-muted-foreground px-2 py-1">
              Loading collections...
            </div>
          )}
          {collectionsQuery.isError && (
            <div className="text-xs text-destructive px-2 py-1">
              Failed to load collections
            </div>
          )}
          {collectionsQuery.isSuccess && collectionsQuery.data.length === 0 && (
            <div className="text-xs text-muted-foreground px-2 py-1">
              No collections found
            </div>
          )}
          {collectionsQuery.isSuccess &&
            collectionsQuery.data.map((collection) => (
              <Button
                key={collection.name}
                variant="ghost"
                className="w-full justify-start h-7 px-2 text-xs font-normal"
                title={collection.name}
              >
                {collection.name}
              </Button>
            ))}
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}

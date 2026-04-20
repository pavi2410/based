import { useQuery } from "@tanstack/react-query";
import { cmd } from "@/commands";
import { 
  FileTextIcon, 
  PlusIcon, 
  StarIcon,
  Loader2Icon,
  RefreshCwIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import type { QuerySummary } from "@/types/project";

interface SavedQueriesListProps {
  projectPath: string;
  connectionKey?: string;
  selectedQuery?: string;
  onSelectQuery: (filename: string) => void;
  onNewQuery: () => void;
}

export function SavedQueriesList({
  projectPath,
  connectionKey,
  selectedQuery,
  onSelectQuery,
  onNewQuery,
}: SavedQueriesListProps) {
  const queriesQuery = useQuery({
    queryKey: ["saved-queries", projectPath],
    queryFn: async () => {
      return await cmd.listSavedQueries(projectPath);
    },
  });

  // Filter queries by connection if specified
  const queries = connectionKey
    ? queriesQuery.data?.filter((q) => q.connection === connectionKey)
    : queriesQuery.data;

  // Group by favorite status
  const favorites = queries?.filter((q) => q.favorite) ?? [];
  const others = queries?.filter((q) => !q.favorite) ?? [];

  if (queriesQuery.status === "pending") {
    return (
      <div className="flex items-center justify-center h-32">
        <Loader2Icon className="size-4 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (queriesQuery.status === "error") {
    return (
      <div className="flex flex-col items-center justify-center h-32 gap-2 px-4">
        <p className="text-xs text-destructive text-center">
          Failed to load queries
        </p>
        <Button 
          variant="outline" 
          size="sm" 
          className="h-6 text-xs"
          onClick={() => queriesQuery.refetch()}
        >
          <RefreshCwIcon className="size-3 mr-1" />
          Retry
        </Button>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header with New Query button */}
      <div className="flex items-center justify-between px-3 py-2 border-b">
        <span className="text-xs font-medium text-muted-foreground">
          Saved Queries
        </span>
        <Button
          variant="ghost"
          size="icon"
          className="size-6"
          onClick={onNewQuery}
        >
          <PlusIcon className="size-3.5" />
        </Button>
      </div>

      <ScrollArea className="flex-1">
        <div className="py-1">
          {/* Favorites Section */}
          {favorites.length > 0 && (
            <div className="mb-2">
              <div className="px-3 py-1">
                <span className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
                  Favorites
                </span>
              </div>
              {favorites.map((query) => (
                <QueryItem
                  key={query.filename}
                  query={query}
                  isSelected={selectedQuery === query.filename}
                  onSelect={() => onSelectQuery(query.filename)}
                />
              ))}
            </div>
          )}

          {/* All Queries Section */}
          {others.length > 0 && (
            <div>
              {favorites.length > 0 && (
                <div className="px-3 py-1">
                  <span className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
                    All Queries
                  </span>
                </div>
              )}
              {others.map((query) => (
                <QueryItem
                  key={query.filename}
                  query={query}
                  isSelected={selectedQuery === query.filename}
                  onSelect={() => onSelectQuery(query.filename)}
                />
              ))}
            </div>
          )}

          {/* Empty State */}
          {queries?.length === 0 && (
            <div className="flex flex-col items-center justify-center py-8 px-4 gap-2">
              <FileTextIcon className="size-8 text-muted-foreground/50" />
              <p className="text-xs text-muted-foreground text-center">
                No saved queries yet
              </p>
              <Button
                variant="outline"
                size="sm"
                className="h-7 text-xs"
                onClick={onNewQuery}
              >
                <PlusIcon className="size-3 mr-1" />
                Create Query
              </Button>
            </div>
          )}
        </div>
      </ScrollArea>
    </div>
  );
}

interface QueryItemProps {
  query: QuerySummary;
  isSelected: boolean;
  onSelect: () => void;
}

function QueryItem({ query, isSelected, onSelect }: QueryItemProps) {
  return (
    <button
      className={cn(
        "w-full flex items-center gap-2 px-3 py-1.5 text-left hover:bg-muted/50 transition-colors",
        isSelected && "bg-muted"
      )}
      onClick={onSelect}
    >
      <FileTextIcon className="size-3.5 shrink-0 text-muted-foreground" />
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1">
          <span className="text-xs font-medium truncate">{query.name}</span>
          {query.favorite && (
            <StarIcon className="size-3 fill-yellow-500 text-yellow-500" />
          )}
        </div>
        {query.description && (
          <p className="text-[10px] text-muted-foreground truncate">
            {query.description}
          </p>
        )}
      </div>
    </button>
  );
}

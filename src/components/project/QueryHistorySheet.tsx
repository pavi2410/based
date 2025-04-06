import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { HistoryIcon, StarIcon, SearchIcon, ClockIcon, TrashIcon, TagIcon, XIcon } from "lucide-react";

import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetFooter,
  SheetTrigger,
} from "@/components/ui/sheet";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { 
  QueryHistoryItem, 
  QueryHistoryFilter,
  getQueryHistory, 
  toggleQueryStar,
  clearQueryHistory,
  deleteQuery,
  updateQueryTags,
  getAllTags
} from "@/stores/query-history";
import { toast } from "sonner";
import { useWorkspace } from "@/contexts/WorkspaceContext";
import { formatDistanceToNow, format } from "date-fns";

export function QueryHistorySheet({ 
  connectionId, 
  children,
}: { 
  connectionId: string,
  children: React.ReactNode
}) {
  const { addTab } = useWorkspace();
  const [searchQuery, setSearchQuery] = useState("");
  const [activeTab, setActiveTab] = useState<"all" | "starred">("all");
  const [selectedTags, setSelectedTags] = useState<string[]>([]);

  // Fetch all available tags
  const { data: allTags = [] } = useQuery<string[]>({
    queryKey: ["queryHistoryTags"],
    queryFn: async () => {
      return await getAllTags();
    },
  });

  // Construct filter based on UI state
  const getFilter = (): QueryHistoryFilter => {
    const filter: QueryHistoryFilter = {
      connectionId,
      search: searchQuery || undefined,
    };
    
    if (activeTab === "starred") {
      filter.isStarred = true;
    }
    
    if (selectedTags.length > 0) {
      filter.tags = selectedTags;
    }
    
    return filter;
  };

  // Fetch query history with filters
  const { data: queryHistory = [], refetch } = useQuery<QueryHistoryItem[]>({
    queryKey: ["queryHistory", connectionId, searchQuery, activeTab, selectedTags],
    queryFn: async () => {
      const history = await getQueryHistory(getFilter());
      return history;
    },
  });

  // Function to toggle star status
  const handleToggleStar = async (queryId: string) => {
    try {
      await toggleQueryStar(queryId);
      refetch();
    } catch (error) {
      toast.error("Error toggling star", {
        description: error instanceof Error ? error.message : "An error occurred",
      });
    }
  };

  // Function to create a new query with selected history item
  const handleUseQuery = (query: string) => {
    addTab("Query", {
      type: "query-view",
      initialQuery: query,
    });
  };

  // Function to delete a single query
  const handleDeleteQuery = async (queryId: string) => {
    try {
      await deleteQuery(queryId);
      refetch();
      toast.success("Query deleted");
    } catch (error) {
      toast.error("Error deleting query", {
        description: error instanceof Error ? error.message : "An error occurred",
      });
    }
  };

  // Function to clear history
  const handleClearHistory = async () => {
    try {
      await clearQueryHistory(connectionId);
      refetch();
      toast.success("History cleared", {
        description: "Query history has been cleared",
      });
    } catch (error) {
      toast.error("Error clearing history", {
        description: error instanceof Error ? error.message : "An error occurred",
      });
    }
  };

  // Function to add a tag to a query
  const handleAddTag = async (queryId: string, tag: string) => {
    try {
      const query = queryHistory.find((q: QueryHistoryItem) => q.id === queryId);
      if (!query) return;
      
      const currentTags = query.tags || [];
      if (currentTags.includes(tag)) return;
      
      await updateQueryTags(queryId, [...currentTags, tag]);
      refetch();
    } catch (error) {
      toast.error("Error adding tag", {
        description: error instanceof Error ? error.message : "An error occurred",
      });
    }
  };

  // Function to remove a tag from a query
  const handleRemoveTag = async (queryId: string, tag: string) => {
    try {
      const query = queryHistory.find((q: QueryHistoryItem) => q.id === queryId);
      if (!query || !query.tags) return;
      
      await updateQueryTags(queryId, query.tags.filter((t: string) => t !== tag));
      refetch();
    } catch (error) {
      toast.error("Error removing tag", {
        description: error instanceof Error ? error.message : "An error occurred",
      });
    }
  };

  // Function to toggle a filter tag
  const handleToggleFilterTag = (tag: string) => {
    setSelectedTags(prev => 
      prev.includes(tag) 
        ? prev.filter(t => t !== tag) 
        : [...prev, tag]
    );
  };

  const formatExecutionTime = (time?: number) => {
    if (!time) return "";
    return time < 1000 
      ? `${time.toFixed(2)}ms` 
      : `${(time / 1000).toFixed(2)}s`;
  };

  return (
    <Sheet>
      <SheetTrigger asChild>
        {children}
      </SheetTrigger>
      <SheetContent className="w-[400px] sm:w-[540px] p-0 flex flex-col">
        <SheetHeader className="p-4 border-b">
          <div className="flex items-center justify-between mr-6">
            <SheetTitle className="flex items-center gap-2">
              <HistoryIcon className="h-5 w-5" />
              Query History
            </SheetTitle>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="ghost" size="icon" className="h-8 w-8">
                    <TrashIcon className="h-4 w-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem 
                    className="text-destructive" 
                    onClick={handleClearHistory}
                  >
                    Clear History
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
          </div>

          <div className="mt-4 space-y-3">
            <div className="relative">
              <SearchIcon className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
              <Input
                placeholder="Search queries..."
                className="pl-8"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
              />
            </div>

            <Tabs
              defaultValue="all"
              className="w-full"
              onValueChange={(value) => setActiveTab(value as "all" | "starred")}
            >
              <TabsList className="grid w-full grid-cols-2 bg-muted-foreground/10">
                <TabsTrigger value="all">All Queries</TabsTrigger>
                <TabsTrigger value="starred">Starred</TabsTrigger>
              </TabsList>
            </Tabs>

            {allTags.length > 0 && (
              <div className="flex flex-wrap gap-1 mt-1">
                {allTags.map((tag: string) => (
                  <Button
                    key={tag}
                    variant={selectedTags.includes(tag) ? "default" : "outline"}
                    size="sm"
                    className="h-6 text-xs rounded-full"
                    onClick={() => handleToggleFilterTag(tag)}
                  >
                    <TagIcon className="h-3 w-3 mr-1" />
                    {tag}
                  </Button>
                ))}
              </div>
            )}
          </div>
        </SheetHeader>
        
        <ScrollArea className="flex-1">
          {queryHistory.length > 0 ? (
            <div className="flex flex-col gap-2 p-4">
              {queryHistory.map((item: QueryHistoryItem) => (
                <div
                  key={item.id}
                  className="group relative rounded-md border p-3 hover:bg-muted/50 cursor-pointer"
                  onClick={() => handleUseQuery(item.query)}
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2 text-sm text-muted-foreground">
                      <ClockIcon className="h-3.5 w-3.5" />
                      <span title={format(item.timestamp, 'PPpp')}>
                        {formatDistanceToNow(item.timestamp, { addSuffix: true })}
                      </span>
                    </div>
                    <div className="flex items-center gap-1">
                      {item.executionTime && (
                        <span className="text-xs text-muted-foreground" title="Execution time">
                          {formatExecutionTime(item.executionTime)}
                        </span>
                      )}
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-6 w-6"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleToggleStar(item.id);
                        }}
                      >
                        <StarIcon
                          className={`h-3.5 w-3.5 ${
                            item.isStarred ? "fill-yellow-400 text-yellow-400" : "text-muted-foreground"
                          }`}
                        />
                      </Button>
                      <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                          <Button
                            variant="ghost" 
                            size="icon"
                            className="h-6 w-6"
                            onClick={(e) => e.stopPropagation()}
                          >
                            <TagIcon className="h-3.5 w-3.5" />
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end" onClick={(e) => e.stopPropagation()}>
                          {allTags.map((tag: string) => (
                            <DropdownMenuItem 
                              key={tag}
                              onClick={() => {
                                if (item.tags?.includes(tag)) {
                                  handleRemoveTag(item.id, tag);
                                } else {
                                  handleAddTag(item.id, tag);
                                }
                              }}
                            >
                              <div className="flex items-center">
                                {item.tags?.includes(tag) ? '✓ ' : ''}
                                {tag}
                              </div>
                            </DropdownMenuItem>
                          ))}
                          <DropdownMenuItem
                            className="text-destructive"
                            onClick={() => handleDeleteQuery(item.id)}
                          >
                            Delete Query
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </div>
                  </div>
                  <div className="mt-2 break-all">
                    <pre className="text-xs font-mono whitespace-pre-wrap line-clamp-3">
                      {item.query}
                    </pre>
                  </div>
                  {item.tags && item.tags.length > 0 && (
                    <div className="mt-2 flex flex-wrap gap-1">
                      {item.tags.map((tag: string) => (
                        <div 
                          key={tag}
                          className="bg-muted text-xs rounded-full px-2 py-0.5 flex items-center"
                        >
                          <TagIcon className="h-3 w-3 mr-1" />
                          {tag}
                        </div>
                      ))}
                    </div>
                  )}
                  {item.resultsCount !== undefined && (
                    <div className="mt-1 text-xs text-muted-foreground">
                      {item.resultsCount} {item.resultsCount === 1 ? 'result' : 'results'}
                    </div>
                  )}
                </div>
              ))}
            </div>
          ) : (
            <div className="flex flex-col items-center justify-center h-full gap-2 p-4 text-center text-muted-foreground">
              <HistoryIcon className="h-10 w-10" />
              <p>No query history found</p>
              {(searchQuery || selectedTags.length > 0) && (
                <p className="text-sm">Try different search criteria</p>
              )}
            </div>
          )}
        </ScrollArea>
        
        <SheetFooter className="border-t p-4">
          <div className="text-xs text-muted-foreground">
            {queryHistory.length} {queryHistory.length === 1 ? 'query' : 'queries'}
          </div>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
} 
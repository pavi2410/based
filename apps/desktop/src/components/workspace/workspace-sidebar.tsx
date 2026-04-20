import { DatabaseIcon, HistoryIcon, FileTextIcon } from "lucide-react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { DatabaseTree } from "./database-tree";
import { SavedQueriesList } from "./saved-queries-list";
import { useConnection } from "@/routes/project.$projectId/conn.$connKey";

interface WorkspaceSidebarProps {
  onDisconnect?: () => void;
  onSelectQuery?: (filename: string) => void;
  onNewQuery?: () => void;
  selectedQuery?: string;
}

export function WorkspaceSidebar({
  onSelectQuery,
  onNewQuery,
  selectedQuery,
}: WorkspaceSidebarProps) {
  const {
    connKey,
    connectionConfig,
    projectPath,
    onSelectTable,
    selectedTable,
    selectedSchema,
  } = useConnection();

  return (
    <div className="flex flex-col h-full border-r bg-background">
      <Tabs defaultValue="database" className="flex flex-col h-full">
        <TabsList className="h-9 w-full justify-start rounded-none border-b bg-transparent px-1 gap-0">
          <Tooltip>
            <TooltipTrigger asChild>
              <TabsTrigger
                value="database"
                className="size-7 p-0 data-[state=active]:bg-muted"
              >
                <DatabaseIcon className="size-4" />
              </TabsTrigger>
            </TooltipTrigger>
            <TooltipContent side="bottom">Database</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <TabsTrigger
                value="queries"
                className="size-7 p-0 data-[state=active]:bg-muted"
              >
                <FileTextIcon className="size-4" />
              </TabsTrigger>
            </TooltipTrigger>
            <TooltipContent side="bottom">Saved Queries</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <TabsTrigger
                value="history"
                className="size-7 p-0 data-[state=active]:bg-muted"
                disabled
              >
                <HistoryIcon className="size-4" />
              </TabsTrigger>
            </TooltipTrigger>
            <TooltipContent side="bottom">History (coming soon)</TooltipContent>
          </Tooltip>
        </TabsList>

        <TabsContent value="database" className="flex-1 m-0 mt-0">
          <ScrollArea className="h-full">
            <DatabaseTree
              connKey={connKey}
              connConfig={connectionConfig}
              projectPath={projectPath}
              onSelectTable={onSelectTable}
              selectedTable={selectedTable}
              selectedSchema={selectedSchema}
            />
          </ScrollArea>
        </TabsContent>

        <TabsContent value="queries" className="flex-1 m-0 mt-0">
          <SavedQueriesList
            projectPath={projectPath}
            connectionKey={connKey}
            selectedQuery={selectedQuery}
            onSelectQuery={onSelectQuery ?? (() => {})}
            onNewQuery={onNewQuery ?? (() => {})}
          />
        </TabsContent>

        <TabsContent value="history" className="flex-1 m-0 mt-0">
          <div className="flex items-center justify-center h-32 text-xs text-muted-foreground">
            Coming soon
          </div>
        </TabsContent>
      </Tabs>
    </div>
  );
}

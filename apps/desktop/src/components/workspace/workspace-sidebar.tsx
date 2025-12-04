import { DatabaseIcon, HistoryIcon, FileTextIcon } from "lucide-react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ScrollArea } from "@/components/ui/scroll-area";
import { DatabaseTree } from "./database-tree";
import { useConnection } from "@/routes/project.$projectId/conn.$connKey";

interface WorkspaceSidebarProps {
  onDisconnect?: () => void;
}

export function WorkspaceSidebar(_props: WorkspaceSidebarProps) {
  const { connKey, connectionConfig, projectPath, onSelectTable, selectedTable, selectedSchema } = useConnection();

  return (
    <div className="flex flex-col h-full border-r bg-background">
      <Tabs defaultValue="database" className="flex flex-col h-full">
        <TabsList className="w-full justify-start rounded-none border-b bg-muted/50 px-2">
          <TabsTrigger value="database" className="gap-2">
            <DatabaseIcon className="size-4" />
            Database
          </TabsTrigger>
          <TabsTrigger value="queries" className="gap-2" disabled>
            <FileTextIcon className="size-4" />
            Queries
          </TabsTrigger>
          <TabsTrigger value="history" className="gap-2" disabled>
            <HistoryIcon className="size-4" />
            History
          </TabsTrigger>
        </TabsList>

        <TabsContent value="database" className="flex-1 m-0">
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

        <TabsContent value="queries" className="flex-1 m-0">
          <div className="flex items-center justify-center h-32 text-sm text-muted-foreground">
            Query files coming in Phase 5
          </div>
        </TabsContent>

        <TabsContent value="history" className="flex-1 m-0">
          <div className="flex items-center justify-center h-32 text-sm text-muted-foreground">
            Query history coming in Phase 8
          </div>
        </TabsContent>
      </Tabs>
    </div>
  );
}

import { TableView } from "@/components/project/TableView.tsx";
import { MongoDBQueryView } from "./MongoDBQueryView";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs.tsx";
import { useWorkspace } from "@/contexts/WorkspaceContext";
import { type DbConnectionMeta } from "@/stores";
import {
  CircleSlash2Icon,
  CodeIcon,
  DatabaseIcon,
  XIcon,
} from "lucide-react";

export function MongoDBWorkspace({ connMeta }: { connMeta: DbConnectionMeta }) {
  const { tabs, activeTab, setActiveTabId, removeTab } = useWorkspace();

  if (!tabs.length || !activeTab) {
    return (
      <div className="flex flex-col items-center justify-center h-full">
        <CircleSlash2Icon className="w-12 h-12 text-muted-foreground/80 mb-4" />
        <h2 className="text-muted-foreground/80 text-lg font-medium">
          No Tabs Open
        </h2>
        <p className="text-muted-foreground/80 font-light mb-4">
          Get started by querying a collection or exploring your data.
        </p>
      </div>
    );
  }

  const activeTabId = activeTab.id;

  return (
    <Tabs
      value={activeTabId}
      onValueChange={setActiveTabId}
      className="h-full gap-0"
    >
      <TabsList
        className="rounded-none border-b overflow-x-scroll w-full bg-transparent"
        style={{
          scrollbarWidth: "thin",
        }}
      >
        {tabs.map((tab) => (
          <TabsTrigger
            key={tab.id}
            value={tab.id}
            className="group relative"
          >
            <span>
              {tab.descriptor.type === "query-view" ? (
                <CodeIcon className="size-4" />
              ) : (
                <DatabaseIcon className="size-4" />
              )}
            </span>
            <span className="group-hover:mask-r-from-[calc(100%-3rem)] group-hover:mask-r-to-100%">{tab.name}</span>
            {/* biome-ignore lint/a11y/useKeyWithClickEvents: <explanation> */}
            <div
              className="absolute right-2 top-1/2 -translate-y-1/2 z-10 hidden group-hover:inline-block text-muted-foreground hover:text-red-500"
              onClick={() => removeTab(tab.id)}
            >
              <XIcon className="size-4" />
            </div>
          </TabsTrigger>
        ))}
      </TabsList>
      <TabsContent value={activeTabId}>
        {activeTab.descriptor.type === "query-view" ? (
          <MongoDBQueryView connection={connMeta} />
        ) : (
          <TableView
            connection={connMeta}
            tableName={activeTab.descriptor.tableName}
          />
        )}
      </TabsContent>
    </Tabs>
  );
} 
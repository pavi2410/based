import { query } from "@/commands.ts";
import { QueryView } from "@/components/project/QueryView.tsx";
import { TableView } from "@/components/project/TableView.tsx";
import { Badge } from "@/components/ui/badge";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb.tsx";
import { Button } from "@/components/ui/button.tsx";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card.tsx";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible.tsx";
import { Separator } from "@/components/ui/separator.tsx";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
  SidebarProvider,
  SidebarRail,
  SidebarTrigger,
} from "@/components/ui/sidebar.tsx";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs.tsx";
import { WorkspaceProvider, useWorkspace } from "@/contexts/WorkspaceContext";
import { type DbConnectionMeta, getConnection } from "@/stores.ts";
import { baseName, buildConnString } from "@/utils";
import { useQuery } from "@tanstack/react-query";
import { Link, createFileRoute } from "@tanstack/react-router";
import {
  ChevronRightIcon,
  CircleSlash2Icon,
  DatabaseIcon,
  ListOrderedIcon,
  MicroscopeIcon,
  NotebookPenIcon,
  RefreshCcwIcon,
  StarIcon,
  Table2Icon,
  TableIcon,
  XIcon,
} from "lucide-react";
import DeviconSqlite from '~icons/devicon/sqlite'
import DeviconMongodb from '~icons/devicon/mongodb'
import type { ReactNode } from "react";
import { useConnection } from "@/queries/use-connection";

export const Route = createFileRoute("/conn/$id")({
  component: RouteComponent,
});

function RouteComponent() {
  const { id } = Route.useParams();
  
  const { status, retry } = useConnection(id);

  // Using exhaustive switch pattern for better type safety
  switch (status.status) {
    case 'loading':
      return <div className="p-2">Loading...</div>;
    case 'error':
      return (
        <div className="flex flex-col items-center justify-center h-full gap-4 p-4">
          <div className="text-destructive text-lg font-medium">Connection Error</div>
          <div className="text-destructive/80 text-center max-w-md">
            {status.error.message}
          </div>
          <div className="flex gap-4 mt-4">
            <Button 
              variant="outline" 
              onClick={retry} 
              className="flex items-center gap-2"
            >
              <RefreshCcwIcon className="size-4" />
              Retry Connection
            </Button>
            <Button asChild>
              <Link to="/">
                Go Home
              </Link>
            </Button>
          </div>
        </div>
      );
    case 'success':
      const connMeta = status.data;
      return (
        <WorkspaceProvider>
          <SidebarProvider>
            <ProjectSidebar connMeta={connMeta} />
            <SidebarInset className="overflow-hidden">
              <ProjectHeader connMeta={connMeta} />
              <ProjectWorkspace connMeta={connMeta} />
            </SidebarInset>
          </SidebarProvider>
        </WorkspaceProvider>
      );
  }
}

function ProjectSidebar({ connMeta }: { connMeta: DbConnectionMeta }) {
  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton>
              {connMeta.dbType === "mongodb" ? (
                <DeviconMongodb />
              ) : (
                <DeviconSqlite />
              )}
              <span>{baseName(connMeta.filePath)}</span>
              <small className="text-muted-foreground">{connMeta.dbType}</small>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <DbObjectMenu
            connMeta={connMeta}
            type="table"
            label="Tables"
            icon={<TableIcon />}
          />
          <DbObjectMenu
            connMeta={connMeta}
            type="view"
            label="Views"
            icon={<Table2Icon />}
          />
          <DbObjectMenu
            connMeta={connMeta}
            type="index"
            label="Indexes"
            icon={<ListOrderedIcon />}
          />
          <DbObjectMenu
            connMeta={connMeta}
            type="trigger"
            label="Triggers"
            icon={<RefreshCcwIcon />}
          />
        </SidebarGroup>
      </SidebarContent>

      <SidebarFooter className="group-data-[collapsible=icon]:hidden">
        <SidebarBranding />
      </SidebarFooter>

      <SidebarRail />
    </Sidebar>
  );
}

function ProjectHeader({ connMeta }: { connMeta: DbConnectionMeta }) {
  const { addTab } = useWorkspace();

  const connName = baseName(connMeta.filePath);

  function addQueryTab() {
    addTab(`Query - ${connName}`, {
      type: "query-view",
    });
  }

  return (
    <header className="flex h-12 shrink-0 items-center gap-2 border-b px-4">
      <SidebarTrigger className="-ml-1" />
      <Separator orientation="vertical" className="mr-2 h-4" />
      <Breadcrumb>
        <BreadcrumbList>
          <BreadcrumbItem className="hidden md:block">
            <BreadcrumbLink asChild>
              <Link to="/">Home</Link>
            </BreadcrumbLink>
          </BreadcrumbItem>
          <BreadcrumbSeparator className="hidden md:block" />
          <BreadcrumbItem>
            <BreadcrumbPage>{connName}</BreadcrumbPage>
          </BreadcrumbItem>
        </BreadcrumbList>
      </Breadcrumb>
      <div className="flex-1" />
      <Button
        variant="outline"
        size="icon"
        title="Query Database"
        onClick={addQueryTab}
      >
        <NotebookPenIcon />
      </Button>
    </header>
  );
}

function ProjectWorkspace({ connMeta }: { connMeta: DbConnectionMeta }) {
  const { tabs, activeTab, setActiveTabId, removeTab } = useWorkspace();

  if (!tabs.length || !activeTab) {
    return (
      <div className="flex flex-col items-center justify-center h-full">
        <CircleSlash2Icon className="w-12 h-12 text-muted-foreground/80 mb-4" />
        <h2 className="text-muted-foreground/80 text-lg font-medium">
          No Tabs Open
        </h2>
        <p className="text-muted-foreground/80 font-light mb-4">
          Get started by querying a database or viewing tables.
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
                <MicroscopeIcon className="size-4" />
              ) : (
                <TableIcon className="size-4" />
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
          <QueryView connection={connMeta} />
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

function DbObjectMenu({
  connMeta,
  type,
  label,
  icon,
}: {
  connMeta: DbConnectionMeta;
  type: string;
  label: string;
  icon: ReactNode;
}) {
  const { addTab } = useWorkspace();
  const connString = buildConnString(connMeta);
  
  // Use the connection hook with connection id
  const { status: connectionStatus, retry } = useConnection(connMeta.id);
  
  const objectQuery = useQuery({
    queryKey: ["connection", connMeta.id, type],
    queryFn: async () => {
      return await query(
        connString,
        `SELECT name
         FROM sqlite_schema
         WHERE type = '${type}'`,
        [],
      );
    },
    enabled: connectionStatus.status === 'success', // Only run when connection is successful
  });

  function addTableTab(tableName: string) {
    addTab(`${tableName}`, {
      type: "table-view",
      tableName,
    });
  }

  // Using exhaustive switch pattern for better type safety
  switch (connectionStatus.status) {
    case 'error':
      return (
        <div className="p-2 border rounded-md bg-muted/20 space-y-2">
          <div className="font-medium text-destructive">Connection Error</div>
          <div className="text-sm text-destructive/80">{connectionStatus.error.message}</div>
          <div className="flex gap-2 pt-1">
            <Button 
              variant="outline" 
              size="sm"
              onClick={retry} 
              className="h-7 text-xs"
            >
              <RefreshCcwIcon className="size-3 mr-1" />
              Retry
            </Button>
            <Button asChild size="sm" variant="secondary" className="h-7 text-xs">
              <Link to="/">
                Go Home
              </Link>
            </Button>
          </div>
        </div>
      );
    case 'loading':
      return <div className="p-2">Connecting to database...</div>;
    case 'success':
      if (objectQuery.isPending) {
        return <div className="p-2">Loading {type}s...</div>;
      }
      
      if (objectQuery.isError) {
        return <div className="p-2 text-destructive">Error: {objectQuery.error.message}</div>;
      }
      
      return (
        <SidebarMenu>
          <Collapsible className="group/collapsible">
            <SidebarMenuItem>
              <CollapsibleTrigger asChild>
                <SidebarMenuButton>
                  {icon}
                  {label}
                  <span className="ml-auto inline-flex items-center gap-1">
                    <Badge variant="outline">{objectQuery.data.length}</Badge>
                    <ChevronRightIcon className="transition-transform duration-200 group-data-[state=open]/collapsible:rotate-90 size-4" />
                  </span>
                </SidebarMenuButton>
              </CollapsibleTrigger>
            </SidebarMenuItem>

            <CollapsibleContent>
              <SidebarMenuSub>
                {objectQuery.data.map((subItem) => (
                  <SidebarMenuSubItem key={subItem.name}>
                    <SidebarMenuSubButton
                      title={subItem.name}
                      onDoubleClick={() => addTableTab(subItem.name)}
                    >
                      <span>{subItem.name}</span>
                    </SidebarMenuSubButton>
                  </SidebarMenuSubItem>
                ))}
              </SidebarMenuSub>
            </CollapsibleContent>
          </Collapsible>
        </SidebarMenu>
      );
  }
}

function SidebarBranding() {
  return (
    <Card className="shadow-none gap-3 py-6">
      <CardHeader>
        <CardTitle className="text-sm text-muted-foreground">pavi2410 / <span className="text-foreground">based</span></CardTitle>
      </CardHeader>
      <CardContent>
        <Button
          className="w-full shadow-none"
          asChild
        >
          <a
            href="https://github.com/pavi2410/based"
            target="_blank"
            rel="noreferrer"
          >
            <StarIcon />
            Star on GitHub
          </a>
        </Button>
      </CardContent>
    </Card>
  );
}

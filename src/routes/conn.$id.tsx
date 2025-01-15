import { load, query } from "@/commands.ts";
import { QueryView } from "@/components/project/QueryView.tsx";
import { TableView } from "@/components/project/TableView.tsx";
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
import { baseName } from "@/utils";
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
import type { ReactNode } from "react";

export const Route = createFileRoute("/conn/$id")({
  component: RouteComponent,
});

function RouteComponent() {
  const { id } = Route.useParams();

  const connQuery = useQuery({
    queryKey: ["conn", id],
    queryFn: async () => {
      return await getConnection(id);
    },
  });

  if (connQuery.status === "pending") {
    return <div className="p-2">Loading...</div>;
  }
  if (connQuery.status === "error") {
    return <div className="p-2">Error: {connQuery.error.message}</div>;
  }
  if (!connQuery.data) {
    return <div className="p-2">Connection not found</div>;
  }

  const conn = connQuery.data;
  return (
    <WorkspaceProvider>
      <SidebarProvider>
        <ProjectSidebar conn={conn} />
        <SidebarInset>
          <ProjectHeader conn={conn} />
          <ProjectWorkspace conn={conn} />
        </SidebarInset>
      </SidebarProvider>
    </WorkspaceProvider>
  );
}

function ProjectSidebar({ conn }: { conn: DbConnectionMeta }) {
  return (
    <Sidebar>
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton>
              <DatabaseIcon />
              <span>{baseName(conn.filePath)}</span>
              <small>{conn.dbType}</small>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <DbObjectMenu
            conn={conn}
            type="table"
            label="Tables"
            icon={<TableIcon />}
          />
          <DbObjectMenu
            conn={conn}
            type="view"
            label="Views"
            icon={<Table2Icon />}
          />
          <DbObjectMenu
            conn={conn}
            type="index"
            label="Indexes"
            icon={<ListOrderedIcon />}
          />
          <DbObjectMenu
            conn={conn}
            type="trigger"
            label="Triggers"
            icon={<RefreshCcwIcon />}
          />
        </SidebarGroup>
      </SidebarContent>

      <SidebarRail />

      <SidebarFooter>
        <SidebarBranding />
      </SidebarFooter>
    </Sidebar>
  );
}

function ProjectHeader({ conn }: { conn: DbConnectionMeta }) {
  const { addTab } = useWorkspace();

  const connName = baseName(conn.filePath);

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
              <Link href="/">Home</Link>
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

function ProjectWorkspace({ conn }: { conn: DbConnectionMeta }) {
  const { tabs, activeTab, setActiveTabId, removeTab } = useWorkspace();

  if (!tabs.length || !activeTab) {
    return (
      <div className="flex flex-col items-center justify-center h-full">
        <CircleSlash2Icon className="w-12 h-12 text-muted-foreground mb-4" />
        <h2 className="text-lg font-medium">No Tabs Open</h2>
        <p className="text-muted-foreground mb-4">
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
      className="h-full flex flex-col"
    >
      <TabsList className="w-full border-b rounded-none justify-start">
        {tabs.map((tab) => (
          <TabsTrigger
            key={tab.id}
            value={tab.id}
            className="gap-2 group items-center"
          >
            <span>
              {tab.descriptor.type === "query-view" ? (
                <MicroscopeIcon className="size-4" />
              ) : (
                <TableIcon className="size-4" />
              )}
            </span>
            <span>{tab.name}</span>
            {/* biome-ignore lint/a11y/useKeyWithClickEvents: <explanation> */}
            <span
              className="absolute right-0 size-4 hidden group-hover:inline-block text-muted-foreground"
              onClick={() => removeTab(tab.id)}
            >
              <XIcon className="size-4" />
            </span>
          </TabsTrigger>
        ))}
      </TabsList>
      <TabsContent value={activeTabId} className="m-0 flex-1">
        {activeTab.descriptor.type === "query-view" ? (
          <QueryView connection={conn} />
        ) : (
          <TableView
            connection={conn}
            tableName={activeTab.descriptor.tableName}
          />
        )}
      </TabsContent>
    </Tabs>
  );
}

function DbObjectMenu({
  conn,
  type,
  label,
  icon,
}: {
  conn: DbConnectionMeta;
  type: string;
  label: string;
  icon: ReactNode;
}) {
  const { addTab } = useWorkspace();
  const objectQuery = useQuery({
    queryKey: ["conn", conn.id, type],
    queryFn: async () => {
      const connString = `sqlite:${conn.filePath}`;
      await load(connString);
      return await query(
        connString,
        `SELECT name
         FROM sqlite_schema
         WHERE type = '${type}'`,
        [],
      );
    },
  });

  function addTableTab(tableName: string) {
    addTab(`Table - ${tableName}`, {
      type: "table-view",
      tableName,
    });
  }

  if (objectQuery.status === "pending") {
    return <div className="p-2">Loading...</div>;
  }
  if (objectQuery.status === "error") {
    return <div className="p-2">Error: {objectQuery.error.message}</div>;
  }

  return (
    <SidebarMenu>
      <Collapsible className="group/collapsible">
        <SidebarMenuItem>
          <CollapsibleTrigger asChild>
            <SidebarMenuButton>
              {icon}
              {label}
              <ChevronRightIcon className="ml-auto transition-transform duration-200 group-data-[state=open]/collapsible:rotate-90" />
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

function SidebarBranding() {
  return (
    <Card className="shadow-none">
      <CardHeader className="p-4 pb-0">
        <CardTitle className="text-sm">pavi2410 / based</CardTitle>
      </CardHeader>
      <CardContent className="grid gap-2.5 p-4">
        <Button
          className="w-full bg-sidebar-primary text-sidebar-primary-foreground shadow-none"
          size="sm"
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

import { query } from "@/commands.ts";
import { Badge } from "@/components/ui/badge";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible.tsx";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
  SidebarRail,
} from "@/components/ui/sidebar.tsx";
import { type DbConnectionMeta } from "@/stores";
import { baseName, buildConnString } from "@/utils";
import { useQuery } from "@tanstack/react-query";
import {
  ChevronRightIcon,
  ListOrderedIcon,
  RefreshCcwIcon,
  Table2Icon,
  TableIcon,
} from "lucide-react";
import DeviconSqlite from '~icons/devicon/sqlite';
import type { ReactNode } from "react";
import { useConnection } from "@/queries/use-connection";
import { useWorkspace } from "@/contexts/WorkspaceContext";
import { EditConnectionDialog } from "@/components/edit-connection-dialogs";
import { DialogTrigger } from "@/components/ui/dialog";

export function SQLiteSidebar({ connMeta }: { connMeta: DbConnectionMeta }) {
  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <EditConnectionDialog connection={connMeta} trigger={
              <DialogTrigger asChild>
                <SidebarMenuButton>
                  <DeviconSqlite />
                  <span>{baseName(connMeta.filePath)}</span>
                </SidebarMenuButton>
              </DialogTrigger>
            } />
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SQLiteObjectMenu
            connMeta={connMeta}
            type="table"
            label="Tables"
            icon={<TableIcon />}
          />
          <SQLiteObjectMenu
            connMeta={connMeta}
            type="view"
            label="Views"
            icon={<Table2Icon />}
          />
          <SQLiteObjectMenu
            connMeta={connMeta}
            type="index"
            label="Indexes"
            icon={<ListOrderedIcon />}
          />
          <SQLiteObjectMenu
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

function SQLiteObjectMenu({
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
  const connString = buildConnString(connMeta);
  const { addTab } = useWorkspace();
  
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

  return (
    <SidebarMenu>
      <Collapsible className="group/collapsible">
        <SidebarMenuItem>
          <CollapsibleTrigger asChild>
            <SidebarMenuButton>
              {icon}
              {label}
              <span className="ml-auto inline-flex items-center gap-1">
                <Badge variant="outline">
                  {objectQuery.isSuccess ? objectQuery.data.length : 0}
                </Badge>
                <ChevronRightIcon className="transition-transform duration-200 group-data-[state=open]/collapsible:rotate-90 size-4" />
              </span>
            </SidebarMenuButton>
          </CollapsibleTrigger>
        </SidebarMenuItem>

        <CollapsibleContent>
          <SidebarMenuSub>
            {objectQuery.isSuccess && objectQuery.data.map((subItem) => (
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
    <div className="px-4 py-2 text-center">
      <small className="text-muted-foreground">SQLite Explorer</small>
    </div>
  );
} 
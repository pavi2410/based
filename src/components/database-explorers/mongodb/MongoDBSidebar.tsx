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
import { type DbConnectionMeta } from "@/stores.ts";
import { buildConnString } from "@/utils";
import { useQuery } from "@tanstack/react-query";
import {
  ChevronRightIcon,
  FolderKanbanIcon,
  KeyIcon,
} from "lucide-react";
import DeviconMongodb from '~icons/devicon/mongodb';
import type { ReactNode } from "react";
import { useConnection } from "@/queries/use-connection";
import { useWorkspace } from "@/contexts/WorkspaceContext";

export function MongoDBSidebar({ connMeta }: { connMeta: DbConnectionMeta }) {
  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton>
              <DeviconMongodb />
              <span>{getMongoDBConnectionName(connMeta.filePath)}</span>
              <small className="text-muted-foreground">{connMeta.dbType}</small>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <MongoDBObjectMenu
            connMeta={connMeta}
            type="collection"
            label="Collections"
            icon={<FolderKanbanIcon />}
          />
          <MongoDBObjectMenu
            connMeta={connMeta}
            type="index"
            label="Indexes"
            icon={<KeyIcon />}
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

function MongoDBObjectMenu({
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
      // For MongoDB, we need different queries based on type
      if (type === "collection") {
        // Query to list collections
        return await query(
          connString,
          JSON.stringify({
            listCollections: 1
          }),
          [],
        );
      } else if (type === "index") {
        // In a real implementation, we would list all indexes across collections
        // This is simplified for now
        return [];
      }
      return [];
    },
    enabled: connectionStatus.status === 'success', // Only run when connection is successful
  });

  function addCollectionTab(collectionName: string) {
    addTab(`${collectionName}`, {
      type: "table-view", // For now, reuse the table view component
      tableName: collectionName, // Use the collection name
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
            {objectQuery.isSuccess && objectQuery.data.map((item: any) => (
              <SidebarMenuSubItem key={item.name}>
                <SidebarMenuSubButton
                  title={item.name}
                  onDoubleClick={() => addCollectionTab(item.name)}
                >
                  <span>{item.name}</span>
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
      <small className="text-muted-foreground">MongoDB Explorer</small>
    </div>
  );
}

// Helper function to extract the database name from MongoDB connection string
function getMongoDBConnectionName(connectionString: string): string {
  try {
    // Parse MongoDB connection string
    const url = new URL(connectionString.replace('mongodb://', 'http://').replace('mongodb+srv://', 'http://'));
    
    // Get database name from path (removing leading slash)
    const dbName = url.pathname.replace('/', '');
    
    if (dbName) {
      return dbName;
    }
    
    // If no database specified, show hostname
    const hostname = url.hostname;
    return hostname || "MongoDB Server";
  } catch (e) {
    // If parsing fails, extract database the basic way
    const parts = connectionString.split('/');
    const lastPart = parts[parts.length - 1];
    
    // If the last part exists and isn't empty, use it
    if (lastPart && lastPart.trim() !== '') {
      return lastPart;
    }
    
    // Otherwise try to extract the host
    try {
      const hostPart = parts[2]; // After mongodb://
      return hostPart.split('@').pop() || "MongoDB Server";
    } catch {
      return "MongoDB Server";
    }
  }
} 
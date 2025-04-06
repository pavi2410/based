import {
  SidebarInset,
  SidebarProvider,
} from "@/components/ui/sidebar.tsx";
import { WorkspaceProvider } from "@/contexts/WorkspaceContext";
import { type DbConnectionMeta } from "@/stores";
import { MongoDBHeader } from "./MongoDBHeader";
import { MongoDBWorkspace } from "./MongoDBWorkspace";
import { MongoDBSidebar } from "./MongoDBSidebar";

export function MongoDBExplorer({ connMeta }: { connMeta: DbConnectionMeta }) {
  return (
    <WorkspaceProvider>
      <SidebarProvider>
        <MongoDBSidebar connMeta={connMeta} />
        <SidebarInset className="overflow-hidden">
          <MongoDBHeader connMeta={connMeta} />
          <MongoDBWorkspace connMeta={connMeta} />
        </SidebarInset>
      </SidebarProvider>
    </WorkspaceProvider>
  );
} 
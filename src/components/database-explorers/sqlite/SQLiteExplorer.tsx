import {
  SidebarInset,
  SidebarProvider,
} from "@/components/ui/sidebar.tsx";
import { WorkspaceProvider } from "@/contexts/WorkspaceContext";
import { type DbConnectionMeta } from "@/stores.ts";
import { SQLiteHeader } from "./SQLiteHeader";
import { SQLiteWorkspace } from "./SQLiteWorkspace";
import { SQLiteSidebar } from "./SQLiteSidebar";

export function SQLiteExplorer({ connMeta }: { connMeta: DbConnectionMeta }) {
  return (
    <WorkspaceProvider>
      <SidebarProvider>
        <SQLiteSidebar connMeta={connMeta} />
        <SidebarInset className="overflow-hidden">
          <SQLiteHeader connMeta={connMeta} />
          <SQLiteWorkspace connMeta={connMeta} />
        </SidebarInset>
      </SidebarProvider>
    </WorkspaceProvider>
  );
} 
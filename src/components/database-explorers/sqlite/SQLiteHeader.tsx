import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb.tsx";
import { Button } from "@/components/ui/button.tsx";
import { Separator } from "@/components/ui/separator.tsx";
import { SidebarTrigger } from "@/components/ui/sidebar.tsx";
import { useWorkspace } from "@/contexts/WorkspaceContext";
import { type SqliteConnectionMeta } from "@/stores/db-connections";
import { baseName } from "@/utils";
import { Link } from "@tanstack/react-router";
import { HistoryIcon, NotebookPenIcon } from "lucide-react";
import { QueryHistorySheet } from "@/components/project/QueryHistorySheet";

export function SQLiteHeader({ connMeta }: { connMeta: SqliteConnectionMeta }) {
  const { addTab } = useWorkspace();
  const connName = baseName(connMeta.filePath);

  function addQueryTab() {
    addTab("Query", {
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
      <div className="flex gap-1">
        <QueryHistorySheet
          connectionId={connMeta.id}
        >
          <Button
            variant="outline"
            size="icon"
            title="Query History"
            className="mr-1"
          >
            <HistoryIcon />
          </Button>
        </QueryHistorySheet>
        <Button
          variant="outline"
          size="icon"
          title="New Query"
          onClick={addQueryTab}
        >
          <NotebookPenIcon />
        </Button>
      </div>
    </header>
  );
} 
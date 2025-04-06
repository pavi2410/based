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
import { type DbConnectionMeta } from "@/stores.ts";
import { Link } from "@tanstack/react-router";
import { CodeIcon } from "lucide-react";

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

export function MongoDBHeader({ connMeta }: { connMeta: DbConnectionMeta }) {
  const { addTab } = useWorkspace();
  const connName = getMongoDBConnectionName(connMeta.filePath);

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
      <Button
        variant="outline"
        size="icon"
        title="Query Database"
        onClick={addQueryTab}
      >
        <CodeIcon />
      </Button>
    </header>
  );
} 
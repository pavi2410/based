import { createFileRoute, Link } from "@tanstack/react-router";
import { Button } from "@/components/ui/button.tsx";
import { getConnections, removeConnection } from "@/stores.ts";
import { useMutation, useQuery } from "@tanstack/react-query";
import {
  DatabaseIcon,
  Loader2Icon,
  PlusIcon,
  StarIcon,
  Trash2Icon,
} from "lucide-react";
import DeviconSqlite from '~icons/devicon/sqlite'
import DeviconMongodb from '~icons/devicon/mongodb'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu.tsx";
import { baseName } from "@/utils";
import { NewConnectionDialog } from "@/components/new-connection-dialog";

export const Route = createFileRoute("/")({
  component: Index,
});

function Index() {
  return (
    <div className="p-4 flex flex-col h-screen">
      <div className="flex justify-between items-center mb-4">
        <Branding />
        <NewConnectionDialog>
          <Button variant="outline" size="icon" title="Add Connection">
            <PlusIcon />
          </Button>
        </NewConnectionDialog>
      </div>

      <div className="flex-1 flex">
        <ConnectionList />
      </div>
    </div>
  );
}

function getConnectionLabel(conn: any) {
  if (conn.dbType === "sqlite") {
    return baseName(conn.filePath);
  } else if (conn.dbType === "mongodb") {
    try {
      // Parse MongoDB connection string
      const url = new URL(conn.filePath.replace('mongodb://', 'http://').replace('mongodb+srv://', 'http://'));
      
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
      const parts = conn.filePath.split('/');
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
  return "Unknown Database";
}

function ConnectionList() {
  const connListQuery = useQuery({
    queryKey: ["connections"],
    queryFn: getConnections,
  });

  const removeConnMutation = useMutation({
    mutationFn: removeConnection,
    onSuccess: async () => {
      await connListQuery.refetch();
    },
  });

  if (connListQuery.status === "pending") return <p className="w-full">Loading...</p>;

  if (connListQuery.status === "error")
    return <p className="w-full">Error: {connListQuery.error.message}</p>;

  if (!connListQuery.data.length) {
    return (
      <div className="flex flex-col items-center justify-center w-full">
        <DatabaseIcon className="size-12 text-muted-foreground/80 mb-6" />
        <h2 className="text-foreground font-semibold mb-2">
          No Connections
        </h2>
        <p className="text-muted-foreground/80 font-normal mb-8 text-center text-sm max-w-md text-balance">
          Get started by adding a database connection using the "New Connection" button.
        </p>
        <NewConnectionDialog>
          <Button>
            <PlusIcon />
            New Connection
          </Button>
        </NewConnectionDialog>
      </div>
    );
  }

  return (
    <div className="grid grid-cols-3 gap-2 w-full">
      {connListQuery.data.map((conn) => (
        <ContextMenu key={conn.groupName}>
          <ContextMenuTrigger>
            <Link to="/conn/$id" params={{ id: conn.id }}>
              <div className="flex flex-col gap-1 p-4 rounded-xl border hover:bg-accent hover:text-accent-foreground">
                <span className="inline-flex items-center gap-2">
                  {conn.dbType === "mongodb" ? (
                    <DeviconMongodb className="text-muted-foreground" />
                  ) : (
                    <DeviconSqlite className="text-muted-foreground" />
                  )}
                  <small className="text-muted-foreground">{conn.dbType}</small>
                </span>
                <div>{getConnectionLabel(conn)}</div>
              </div>
            </Link>
          </ContextMenuTrigger>
          <ContextMenuContent>
            <ContextMenuItem
              className="text-red-500!"
              disabled={removeConnMutation.isPending}
              onClick={() => removeConnMutation.mutate(conn.id)}
            >
              {removeConnMutation.isPending ? (
                <Loader2Icon className="animate-spin" />
              ) : (
                <Trash2Icon className="size-4" />
              )}
              &nbsp; Remove
            </ContextMenuItem>
          </ContextMenuContent>
        </ContextMenu>
      ))}
    </div>
  );
}

function Branding() {
  return (
    <div className="flex flex-row items-center">
      <div className="mr-4">
        <h1 className="text-sm font-medium">
          <span className="text-muted-foreground">pavi2410 / </span>
          <span className="text-foreground font-bold">based</span>
        </h1>
        <em className="text-xs text-muted-foreground block">The Everything Database App</em>
      </div>
      <div className="flex items-center gap-3 border-l pl-4">
        <Button
          className="shadow-none text-muted-foreground hover:text-primary hover:bg-primary/10"
          variant="outline"
          size="sm"
          asChild
        >
          <a
            href="https://github.com/pavi2410/based"
            target="_blank"
            rel="noreferrer"
          >
            <StarIcon className="mr-1 h-3.5 w-3.5" />
            Star on GitHub
          </a>
        </Button>
      </div>
    </div>
  );
}

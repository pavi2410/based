import { createFileRoute, Link } from "@tanstack/react-router";
import { Button } from "@/components/ui/button.tsx";
import { ConnectionMeta } from "@/stores/db-connections";
import {
  DatabaseIcon,
  Loader2Icon,
  PlusIcon,
  StarIcon,
  Trash2Icon,
  Pencil,
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
import { EditConnectionDialog } from "@/components/edit-connection-dialogs/index.tsx";
import { DialogTrigger } from "@/components/ui/dialog";
import { useRemoveConnectionMutation } from "@/mutations/remove-connection";
import { useConnectionList } from "@/queries/connection-list";

export const Route = createFileRoute("/")({
  component: Index,
});

function Index() {
  return (
    <div className="flex flex-col h-screen overflow-hidden">
      <div className="flex justify-between items-center border-b p-4">
        <Branding />
        <NewConnectionDialog>
          <Button variant="outline" size="icon" title="Add Connection">
            <PlusIcon />
          </Button>
        </NewConnectionDialog>
      </div>

      <div className="flex-1 flex p-4 overflow-y-auto">
        <ConnectionList />
      </div>
    </div>
  );
}

function getConnectionLabel(conn: ConnectionMeta) {
  if (conn.dbType === "sqlite") {
    return baseName(conn.filePath);
  } else if (conn.dbType === "mongodb") {
    try {
      // Parse MongoDB connection string
      const url = new URL(conn.connectionString.replace('mongodb://', 'http://').replace('mongodb+srv://', 'http://'));

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
      const parts = conn.connectionString.split('/');
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
  const connListQuery = useConnectionList();

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
    <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 grid-flow-row auto-rows-min gap-4 w-full">
      {connListQuery.data.map((conn) => (
        <EditConnectionDialog key={conn.id} connection={conn} trigger={<ConnectionItem connection={conn} />} />
      ))}
    </div>
  );
}

function ConnectionItem({
  connection,
}: {
  connection: ConnectionMeta,
}) {
  const removeConnMutation = useRemoveConnectionMutation();

  return (
    <ContextMenu key={connection.id}>
      <ContextMenuTrigger asChild>
        <Link to="/conn/$id" params={{ id: connection.id }}>
          <div className="flex flex-col gap-1 p-4 rounded-xl border hover:bg-accent hover:text-accent-foreground">
            <span className="inline-flex items-center gap-2">
              {connection.dbType === "mongodb" ? (
                <DeviconMongodb className="text-muted-foreground" />
              ) : (
                <DeviconSqlite className="text-muted-foreground" />
              )}
              <small className="text-muted-foreground">{connection.dbType}</small>
            </span>
            <div>{getConnectionLabel(connection)}</div>
          </div>
        </Link>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <DialogTrigger asChild>
          <ContextMenuItem>
            <Pencil className="size-4" />
            &nbsp; Edit
          </ContextMenuItem>
        </DialogTrigger>
        <ContextMenuItem
          className="text-red-500!"
          disabled={removeConnMutation.isPending}
          onClick={() => removeConnMutation.mutate(connection.id)}
        >
          {removeConnMutation.isPending ? (
            <Loader2Icon className="animate-spin size-4" />
          ) : (
            <Trash2Icon className="size-4 text-red-500" />
          )}
          &nbsp; Remove
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
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

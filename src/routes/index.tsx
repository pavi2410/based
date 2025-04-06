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
                <DatabaseIcon className="text-muted-foreground" />
                <div>{baseName(conn.filePath)}</div>
                <small className="text-muted-foreground">{conn.dbType}</small>
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
    <div className="flex flex-row gap-2 justify-center items-center">
      <h1 className="font-semibold">based</h1>
      <span className="text-sm text-muted-foreground">by pavi2410</span>
      <Button
        className="shadow-none text-muted-foreground bg-background"
        variant="outline"
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
    </div>
  );
}

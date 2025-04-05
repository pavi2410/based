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
    <div className="p-4 flex flex-col gap-4">
      <div className="flex justify-between items-center">
        <Branding />
        <NewConnectionDialog>
          <Button variant="outline" size="icon" title="Add Connection">
            <PlusIcon />
          </Button>
        </NewConnectionDialog>
      </div>

      <ConnectionList />
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

  if (connListQuery.status === "pending") return <p>Loading...</p>;

  if (connListQuery.status === "error")
    return <p>Error: {connListQuery.error.message}</p>;

  return (
    <div className="grid grid-cols-3 gap-2">
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

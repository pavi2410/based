import { createFileRoute, Link } from "@tanstack/react-router";
import { Button } from "@/components/ui/button.tsx";
import { toast } from "@/hooks/use-toast.ts";
import { addConnection, getConnections, removeConnection } from "@/stores.ts";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
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
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog.tsx";
import { Input } from "@/components/ui/input.tsx";
import { Label } from "@/components/ui/label.tsx";
import { SelectFile } from "@/components/select-file";
import { baseName } from "@/utils";

export const Route = createFileRoute("/")({
  component: Index,
});

function Index() {
  const queryClient = useQueryClient();
  const newConnMutation = useMutation({
    mutationFn: async ({
      dbType,
      filePath,
    }: {
      dbType: string;
      filePath: string;
    }) => {
      await addConnection({
        dbType: dbType as "sqlite",
        filePath,
        groupName: "test",
      });
    },
    onSuccess: async () => {
      toast({
        title: "New connected added",
      });
      await queryClient.invalidateQueries({
        queryKey: ["connections"],
      });
    },
  });

  return (
    <div className="p-4 flex flex-col gap-4">
      <div className="flex justify-between items-center">
        <Branding />
        <Dialog>
          <DialogTrigger asChild>
            <Button variant="outline" size="icon" title="Add Connection">
              <PlusIcon />
            </Button>
          </DialogTrigger>
          <DialogContent className="sm:max-w-[425px]">
            <DialogHeader>
              <DialogTitle>Add New Connection</DialogTitle>
              <DialogDescription>
                Add a new connection to a database.
              </DialogDescription>
            </DialogHeader>
            <form
              id="new-connection-form"
              onSubmit={(e) => {
                e.preventDefault();
                e.stopPropagation();

                const formData = new FormData(e.currentTarget);
                const dbType = formData.get("dbType") as string;
                const filePath = formData.get("filePath") as string;
                if (!dbType || !filePath) return;
                newConnMutation.mutate({
                  dbType,
                  filePath,
                });
              }}
            >
              <div className="grid gap-4 py-4">
                <div className="grid grid-cols-4 items-center gap-4">
                  <Label htmlFor="dbType" className="text-right text-nowrap">
                    Database
                  </Label>
                  <Input
                    id="dbType"
                    name="dbType"
                    value="sqlite"
                    readOnly
                    className="col-span-3"
                  />
                  <Label htmlFor="filePath" className="text-right text-nowrap">
                    File Path
                  </Label>
                  <div className="col-span-3">
                    <SelectFile />
                  </div>
                </div>
              </div>
            </form>
            <DialogFooter>
              <DialogClose asChild>
                <Button type="submit" form="new-connection-form">
                  Add Connection
                </Button>
              </DialogClose>
            </DialogFooter>
          </DialogContent>
        </Dialog>
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

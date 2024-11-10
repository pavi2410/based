import {createLazyFileRoute} from '@tanstack/react-router'
import {ResizableHandle, ResizablePanel, ResizablePanelGroup} from "@/components/ui/resizable.tsx";
import {useMutation, useQuery, useQueryClient} from "@tanstack/react-query";
import {addConnection, getConnections, removeConnection} from "@/stores.ts";
import {toast} from "@/hooks/use-toast.ts";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger
} from "@/components/ui/dialog.tsx";
import {Button} from "@/components/ui/button.tsx";
import {Label} from "@/components/ui/label.tsx";
import {Input} from "@/components/ui/input.tsx";
import {InfoIcon, Loader2Icon, PlusIcon, Trash2Icon, XIcon} from "lucide-react";
import {open} from "@tauri-apps/plugin-dialog";
import {useState} from "react";
import {ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger} from "@/components/ui/context-menu.tsx";

export const Route = createLazyFileRoute('/project/$id')({
  component: RouteComponent,
})

function RouteComponent() {
  const {id} = Route.useParams()
  return (
    <ResizablePanelGroup direction="horizontal" style={{height: 'calc(100% - 40px)'}}>
      <ResizablePanel defaultSize={25} minSize={10} maxSize={40}>
        <DatabaseTreeControls projectId={id}/>
        <DatabaseTree projectId={id}/>
      </ResizablePanel>
      <ResizableHandle withHandle/>
      <ResizablePanel>
        <div className="p-2">
          Hello /project/{id}
        </div>
      </ResizablePanel>
    </ResizablePanelGroup>
  )
}

function DatabaseTreeControls({projectId}: { projectId: string }) {
  const queryClient = useQueryClient()
  const newConnectionMutation = useMutation({
    mutationFn: async ({dbType, filePath}: { dbType: string, filePath: string }) => {
      await addConnection(projectId, {
        dbType: dbType as 'sqlite',
        filePath,
      })
    },
    onSuccess: async () => {
      toast({
        title: 'New connected added',
      });
      await queryClient.invalidateQueries({queryKey: ['projects', projectId, 'connections']})
    }
  })
  return (
    <div className="border-b">
      <Dialog>
        <DialogTrigger asChild>
          <Button size="icon" variant="ghost" title="New connection">
            <PlusIcon/>
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
              e.preventDefault()
              e.stopPropagation()

              const formData = new FormData(e.currentTarget);
              console.log('formData', formData, JSON.stringify(Object.fromEntries(formData)));
              const dbType = formData.get('dbType') as string;
              const filePath = formData.get('filePath') as string;
              if (!dbType || !filePath) return;
              newConnectionMutation.mutate({
                dbType,
                filePath,
              })
            }}
          >
            <div className="grid gap-4 py-4">
              <div className="grid grid-cols-4 items-center gap-4">
                <Label htmlFor="dbType" className="text-right text-nowrap">
                  Database
                </Label>
                <Input id="dbType" name="dbType" value="sqlite" readOnly className="col-span-3"/>
                <Label htmlFor="filePath" className="text-right text-nowrap">
                  File Path
                </Label>
                <div className="col-span-3">
                  <SelectFile/>
                </div>
              </div>
            </div>
          </form>
          <DialogFooter>
            <DialogClose asChild>
              <Button type="submit" form="new-connection-form">Add Connection</Button>
            </DialogClose>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}

function SelectFile() {
  const [filePath, setFilePath] = useState<string | null>(null)

  if (filePath) {
    return (
      <div className="relative">
        <Button
          variant="ghost"
          size="icon"
          onClick={() => setFilePath(null)}
          className="absolute right-1 top-1.5 size-6"
        >
          <XIcon/>
        </Button>
        <Input readOnly name="filePath" value={filePath} className="pr-8"/>
      </div>

    )
  }

  return (
    <Button onClick={async () => {
      const path = await open({
        title: 'Select a SQLite file',
        filters: [{
          name: 'SQLite files',
          extensions: ['db', 'sqlite', 'sqlite3'],
        }],
        multiple: false,
        directory: false,
      })
      setFilePath(path)
    }}>
      Select File
    </Button>
  )
}

function DatabaseTree({projectId}: { projectId: string }) {
  const connectionsQuery = useQuery({
    queryKey: ['projects', projectId, 'connections'],
    queryFn: async () => {
      return await getConnections(projectId);
    }
  })
  const removeConnectionMutation = useMutation({
    mutationFn: async (connectionId: string) => {
      await removeConnection(projectId, connectionId)
    },
    onSuccess: async () => {
      await connectionsQuery.refetch()
    }
  })
  if (connectionsQuery.status === 'pending') {
    return (
      <div className="p-2">
        Loading...
      </div>
    )
  }
  if (connectionsQuery.status === 'error') {
    return (
      <div className="p-2">
        Error: {connectionsQuery.error.message}
      </div>
    )
  }
  return (
    <div className="p-2">
      {connectionsQuery.data.map(connection => (
        <Dialog key={connection.id}>
          <ContextMenu>
            <ContextMenuTrigger>
              <div className="p-2 rounded hover:bg-accent">
                {connection.filePath.replace(/^.+[\/\\]/, '')}
                <br/>
                <small>{connection.dbType}</small>
              </div>
            </ContextMenuTrigger>
            <ContextMenuContent>
              <DialogTrigger asChild>
                <ContextMenuItem>
                  <InfoIcon className="size-4"/>
                  &nbsp;
                  Info
                </ContextMenuItem>
              </DialogTrigger>
              <ContextMenuItem
                className="!text-red-500"
                onClick={() => removeConnectionMutation.mutate(connection.id)}
              >
                {removeConnectionMutation.isPending ? (
                  <Loader2Icon className="animate-spin"/>
                ) : (
                  <Trash2Icon className="size-4"/>
                )}
                &nbsp;
                Remove
              </ContextMenuItem>
            </ContextMenuContent>
          </ContextMenu>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Connection Info</DialogTitle>
            </DialogHeader>
            <div className="grid grid-cols-4 gap-y-2">
              <div>Database</div>
              <div className="col-span-3 font-bold">
                {connection.dbType}
              </div>
              <div>File Path</div>
              <div className="col-span-3 font-bold">
                {connection.filePath}
              </div>
            </div>
          </DialogContent>
        </Dialog>
      ))}
    </div>
  )
}
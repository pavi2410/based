import {createLazyFileRoute, Link} from '@tanstack/react-router'
import {useMutation, useQuery, useQueryClient} from "@tanstack/react-query";
import {addConnection, getConnections, getProject, removeConnection} from "@/stores.ts";
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
import {HomeIcon, Loader2Icon, MoreHorizontal, PlusIcon, StarIcon, XIcon} from "lucide-react";
import {open} from "@tauri-apps/plugin-dialog";
import {useState} from "react";
import {load, query} from "@/commands.ts";
import {Textarea} from "@/components/ui/textarea.tsx";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupAction,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarRail,
  SidebarTrigger
} from "@/components/ui/sidebar.tsx";
import {Separator} from "@/components/ui/separator.tsx";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator
} from "@/components/ui/breadcrumb.tsx";
import {Card, CardContent, CardDescription, CardHeader, CardTitle} from "@/components/ui/card.tsx";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger
} from "@/components/ui/dropdown-menu.tsx";
import {Skeleton} from "@/components/ui/skeleton.tsx";

export const Route = createLazyFileRoute('/project/$id')({
  component: RouteComponent,
})

function RouteComponent() {
  const {id} = Route.useParams()
  return (
    <SidebarProvider>
      <ProjectSidebar projectId={id}/>
      <SidebarInset>
        <ProjectHeader projectId={id}/>
        <Test projectId={id}/>
      </SidebarInset>
    </SidebarProvider>
  )
}

function ProjectSidebar({projectId}: { projectId: string }) {
  return (
    <Sidebar>
      <SidebarHeader>
        <SidebarMenuButton size="lg" asChild>
          <Link href="/">
            <HomeIcon/>
            <span>Home</span>
          </Link>
        </SidebarMenuButton>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>DB Connections</SidebarGroupLabel>
          <DatabaseTreeControls projectId={projectId}/>
          <DatabaseTree projectId={projectId}/>
        </SidebarGroup>
      </SidebarContent>
      <SidebarRail/>
      <SidebarFooter>
        <SidebarBranding/>
      </SidebarFooter>
    </Sidebar>
  )
}

function ProjectHeader({projectId}: { projectId: string }) {
  const projectQuery = useQuery({
    queryKey: ['projects', projectId],
    queryFn: async () => {
      return await getProject(projectId);
    }
  })
  return (
    <header className="flex h-16 shrink-0 items-center gap-2 border-b px-4">
      <SidebarTrigger className="-ml-1"/>
      <Separator orientation="vertical" className="mr-2 h-4"/>
      <Breadcrumb>
        <BreadcrumbList>
          <BreadcrumbItem className="hidden md:block">
            <BreadcrumbLink asChild>
              <Link href="/">Projects</Link>
            </BreadcrumbLink>
          </BreadcrumbItem>
          <BreadcrumbSeparator className="hidden md:block"/>
          <BreadcrumbItem>
            <BreadcrumbPage>
              {projectQuery.data?.name ?? <Skeleton className="h-4 w-24"/>}
            </BreadcrumbPage>
          </BreadcrumbItem>
        </BreadcrumbList>
      </Breadcrumb>
    </header>
  )
}

function Test({projectId}: { projectId: string }) {
  const connectionQuery = useQuery({
    queryKey: ['projects', projectId, 'connections', 'first'],
    queryFn: async () => {
      return (await getConnections(projectId))[0];
    }
  })
  if (connectionQuery.status === 'pending') {
    return (
      <div className="p-2">
        Loading...
      </div>
    )
  }
  if (connectionQuery.status === 'error') {
    return (
      <div className="p-2">
        Error: {connectionQuery.error.message}
      </div>
    )
  }
  return (
    <div className="p-2">
      {connectionQuery.data.filePath}
      <QueryTest dbPath={connectionQuery.data.filePath}/>
    </div>
  )
}

function QueryTest({dbPath}: { dbPath: string }) {
  const [connected, setConnected] = useState(false)
  const connectMutation = useMutation({
    mutationFn: async () => {
      const ret = await load(`sqlite:${dbPath}`)
      console.log(ret);
    },
    onSuccess: () => {
      setConnected(true)
      toast({
        title: 'Connected',
      })
    },
    onError: (err) => {
      setConnected(false)
      toast({
        title: 'Error',
        description: err.message,
      })
      console.log(err)
    }
  })

  if (!connected) {
    return (
      <div>
        <Button
          disabled={connectMutation.isPending}
          onClick={() => connectMutation.mutate()}
        >
          {connectMutation.isPending && <Loader2Icon className="animate-spin"/>}
          Connect
        </Button>
      </div>
    )
  }

  return (
    <div>
      Connected!
      <ConnectedTest dbPath={dbPath}/>
    </div>
  )
}

function ConnectedTest({dbPath}: { dbPath: string }) {
  const [queryText, setQueryText] = useState('')

  const queryMutation = useMutation({
    mutationFn: async () => {
      const ret = await query(`sqlite:${dbPath}`, queryText, [])
      console.log(ret);
      return ret;
    },
    onSuccess: () => {
      toast({
        title: 'Executed',
      })
    },
    onError: (err) => {
      console.log('query', err)
    }
  })

  return (
    <div>
      <Textarea value={queryText} onChange={e => setQueryText(e.target.value)}/>
      <Button
        disabled={queryMutation.isPending}
        onClick={() => queryMutation.mutate()}
      >
        {queryMutation.isPending && <Loader2Icon className="animate-spin"/>}
        Run Query
      </Button>
      <Textarea value={JSON.stringify(queryMutation.data, null, 2)} readOnly/>
    </div>
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
    <Dialog>
      <DialogTrigger asChild>
        <SidebarGroupAction title="Add Connection">
          <PlusIcon/> <span className="sr-only">Add Connection</span>
        </SidebarGroupAction>
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
    <SidebarGroupContent>
      <SidebarMenu>
        {connectionsQuery.data.map(connection => (
          <Dialog key={connection.id}>
            <SidebarMenuItem>
              <SidebarMenuButton>
                {connection.filePath.replace(/^.+[\/\\]/, '')}
                <small>{connection.dbType}</small>
              </SidebarMenuButton>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <SidebarMenuAction>
                    <MoreHorizontal/>
                  </SidebarMenuAction>
                </DropdownMenuTrigger>
                <DropdownMenuContent side="right" align="start">
                  <DialogTrigger asChild>
                    <DropdownMenuItem>
                      <span>Edit</span>
                    </DropdownMenuItem>
                  </DialogTrigger>
                  <DropdownMenuItem onClick={() => removeConnectionMutation.mutate(connection.id)}>
                    <span>Remove</span>
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </SidebarMenuItem>
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
      </SidebarMenu>
    </SidebarGroupContent>
  )
}

function SidebarBranding() {
  return (
    <Card className="shadow-none">
      <CardHeader className="p-4 pb-0">
        <CardTitle className="text-sm">pavi2410/based</CardTitle>
        <CardDescription>
          Free & Open Source DataGrip alternative.
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-2.5 p-4">
        <Button
          className="w-full bg-sidebar-primary text-sidebar-primary-foreground shadow-none"
          size="sm"
          asChild
        >
          <a href="https://github.com/pavi2410/based" target="_blank">
            <StarIcon/>
            Star on GitHub
          </a>
        </Button>
      </CardContent>
    </Card>
  )
}
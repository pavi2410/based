import {createFileRoute, Link} from "@tanstack/react-router";
import {useMutation, useQuery, useQueryClient} from "@tanstack/react-query";
import {addConnection, DbConnectionMeta, getConnections, getProject, removeConnection} from "@/stores.ts";
import {toast} from "@/hooks/use-toast.ts";
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
import {Button} from "@/components/ui/button.tsx";
import {Label} from "@/components/ui/label.tsx";
import {Input} from "@/components/ui/input.tsx";
import {
  ChevronRightIcon,
  CircleSlash2Icon,
  DatabaseIcon,
  HomeIcon,
  ListOrderedIcon,
  MicroscopeIcon,
  MoreHorizontal,
  PlusIcon,
  RefreshCcwIcon,
  StarIcon,
  Table2Icon,
  TableIcon,
  XIcon,
} from "lucide-react";
import {open} from "@tauri-apps/plugin-dialog";
import {ReactNode, useEffect, useMemo, useState} from "react";
import {load, query} from "@/commands.ts";
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
  SidebarMenuSub,
  SidebarProvider,
  SidebarRail,
  SidebarTrigger,
} from "@/components/ui/sidebar.tsx";
import {Separator} from "@/components/ui/separator.tsx";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb.tsx";
import {Card, CardContent, CardDescription, CardHeader, CardTitle,} from "@/components/ui/card.tsx";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu.tsx";
import {Skeleton} from "@/components/ui/skeleton.tsx";
import {Collapsible, CollapsibleContent, CollapsibleTrigger,} from "@/components/ui/collapsible.tsx";
import {ProjectWorkspaceProvider, useProjectWorkspace} from "@/contexts/ProjectWorkspaceContext.tsx";
import {Tabs, TabsContent, TabsList, TabsTrigger} from "@/components/ui/tabs.tsx";
import {QueryView} from "@/components/project/QueryView.tsx";

export const Route = createFileRoute('/project/$id')({
  component: RouteComponent,
})

function RouteComponent() {
  const {id} = Route.useParams()
  return (
    <ProjectWorkspaceProvider>
      <SidebarProvider>
        <ProjectSidebar projectId={id}/>
        <SidebarInset>
          <ProjectHeader projectId={id}/>
          <ProjectWorkspace projectId={id}/>
        </SidebarInset>
      </SidebarProvider>
    </ProjectWorkspaceProvider>
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
      return await getProject(projectId)
    },
  })
  return (
    <header className="flex h-12 shrink-0 items-center gap-2 border-b px-4">
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

function ProjectWorkspace({projectId}: { projectId: string }) {
  const {tabs} = useProjectWorkspace()
  const [activeTabId, setActiveTabId] = useState(tabs[0]?.id)

  const activeTab = useMemo(() => tabs.find(t => t.id === activeTabId), [tabs, activeTabId])

  useEffect(() => {
    if (tabs.length) {
      setActiveTabId(tabs[tabs.length - 1].id)
    }
  }, [tabs])

  if (!tabs.length || !activeTabId || !activeTab) {
    return (
      <div className="flex flex-col items-center justify-center h-full">
        <CircleSlash2Icon className="w-12 h-12 text-muted-foreground mb-4"/>
        <h2 className="text-lg font-medium">No Tabs Open</h2>
        <p className="text-muted-foreground mb-4">Get started by querying a database or viewing tables.</p>
      </div>
    )
  }

  return (
    <Tabs value={activeTabId} onValueChange={setActiveTabId} className="h-full flex flex-col">
      <TabsList className="w-full border-b rounded-none justify-start">
        {tabs.map((tab) => (
          <TabsTrigger
            key={tab.id}
            value={tab.id}
            className="gap-2 group items-center"
          >
            <span>
              {tab.descriptor.type === 'query-view' ? <MicroscopeIcon className="size-4"/> :
                <TableIcon className="size-4"/>}
            </span>
            <span>{tab.name}</span>
            <span className="size-4 hidden group-hover:inline-block text-muted-foreground">
              <XIcon className="size-4"/>
            </span>
          </TabsTrigger>
        ))}
      </TabsList>
      <TabsContent value={activeTabId} className="m-0 flex-1">
        {
          activeTab.descriptor.type === 'query-view' ? (
            <QueryView projectId={projectId} connectionId={activeTab.descriptor.connectionId}/>
          ): (
            <>
              <pre>{JSON.stringify(activeTab, null, 2)}</pre>
            </>
          )
        }
      </TabsContent>
    </Tabs>
  )
}


function DatabaseTreeControls({projectId}: { projectId: string }) {
  const queryClient = useQueryClient()
  const newConnectionMutation = useMutation({
    mutationFn: async ({
                         dbType,
                         filePath,
                       }: {
      dbType: string
      filePath: string
    }) => {
      await addConnection(projectId, {
        dbType: dbType as 'sqlite',
        filePath,
      })
    },
    onSuccess: async () => {
      toast({
        title: 'New connected added',
      })
      await queryClient.invalidateQueries({
        queryKey: ['projects', projectId, 'connections'],
      })
    },
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

            const formData = new FormData(e.currentTarget)
            console.log(
              'formData',
              formData,
              JSON.stringify(Object.fromEntries(formData)),
            )
            const dbType = formData.get('dbType') as string
            const filePath = formData.get('filePath') as string
            if (!dbType || !filePath) return
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
                <SelectFile/>
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
    <Button
      onClick={async () => {
        const path = await open({
          title: 'Select a SQLite file',
          filters: [
            {
              name: 'SQLite files',
              extensions: ['db', 'sqlite', 'sqlite3'],
            },
          ],
          multiple: false,
          directory: false,
        })
        setFilePath(path)
      }}
    >
      Select File
    </Button>
  )
}

function DatabaseTree({projectId}: { projectId: string }) {
  const connectionsQuery = useQuery({
    queryKey: ['projects', projectId, 'connections'],
    queryFn: async () => {
      return await getConnections(projectId)
    },
  })
  if (connectionsQuery.status === 'pending') {
    return <div className="p-2">Loading...</div>
  }
  if (connectionsQuery.status === 'error') {
    return <div className="p-2">Error: {connectionsQuery.error.message}</div>
  }
  return (
    <SidebarGroupContent>
      <SidebarMenu>
        {connectionsQuery.data.map((connection) => (
          <DatabaseTreeItem
            projectId={projectId}
            connection={connection}
            key={connection.id}
          />
        ))}
      </SidebarMenu>
    </SidebarGroupContent>
  )
}

function DatabaseTreeItem({
                            projectId,
                            connection,
                          }: {
  projectId: string
  connection: DbConnectionMeta
}) {
  const {addTab} = useProjectWorkspace()
  const queryClient = useQueryClient()
  const connectMutation = useMutation({
    mutationFn: async () => {
      const connString = `sqlite:${connection.filePath}`
      const ret = await load(connString)
      console.log(ret)
    },
    onSuccess: () => {
      toast({
        title: `Connected to database: ${baseName(connection.filePath)}`,
      })
    },
    onError: (err) => {
      toast({
        title: `Error connecting to database: ${baseName(connection.filePath)}`,
        description: err.message,
      })
      console.log(err)
    },
  })
  const removeConnectionMutation = useMutation({
    mutationFn: async () => {
      await removeConnection(projectId, connection.id)
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ['projects', projectId, 'connections'],
      })
    },
  })

  return (
    <Dialog>
      <SidebarMenuItem>
        <Collapsible className="group/collapsible [&[data-state=open]>button>svg:first-child]:rotate-90">
          <CollapsibleTrigger asChild>
            <SidebarMenuButton>
              <ChevronRightIcon className="transition-transform"/>
              <DatabaseIcon/>
              <span>{baseName(connection.filePath)}</span>
              <small>{connection.dbType}</small>
            </SidebarMenuButton>
          </CollapsibleTrigger>
          <CollapsibleContent>
            <SidebarMenuSub>
              <DbObjectMenu
                projectId={projectId}
                connection={connection}
                type="table"
                label="Tables"
                icon={<TableIcon/>}
              />
              <DbObjectMenu
                projectId={projectId}
                connection={connection}
                type="view"
                label="Views"
                icon={<Table2Icon/>}
              />
              <DbObjectMenu
                projectId={projectId}
                connection={connection}
                type="index"
                label="Indexes"
                icon={<ListOrderedIcon/>}
              />
              <DbObjectMenu
                projectId={projectId}
                connection={connection}
                type="trigger"
                label="Triggers"
                icon={<RefreshCcwIcon/>}
              />
            </SidebarMenuSub>
          </CollapsibleContent>
        </Collapsible>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <SidebarMenuAction>
              <MoreHorizontal/>
            </SidebarMenuAction>
          </DropdownMenuTrigger>
          <DropdownMenuContent side="right" align="start" className="*:cursor-pointer">
            <DropdownMenuItem onClick={() => connectMutation.mutate()}>
              <span>Connect</span>
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => addTab(`Query - ${baseName(connection.filePath)}`, {
              type: 'query-view',
              connectionId: connection.id,
            })}>
              <span>Query</span>
            </DropdownMenuItem>
            <DialogTrigger asChild>
              <DropdownMenuItem>
                <span>Edit</span>
              </DropdownMenuItem>
            </DialogTrigger>
            <DropdownMenuItem onClick={() => removeConnectionMutation.mutate()}>
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
          <div className="col-span-3 font-bold">{connection.dbType}</div>
          <div>File Path</div>
          <div className="col-span-3 font-bold">{connection.filePath}</div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function DbObjectMenu({
                        projectId,
                        connection,
                        type,
                        label,
                        icon,
                      }: {
  projectId: string
  connection: DbConnectionMeta
  type: string
  label: string
  icon: ReactNode
}) {
  const {addTab} = useProjectWorkspace()
  const objectQuery = useQuery({
    queryKey: ['projects', projectId, 'connections', connection.id, type],
    queryFn: async () => {
      const connString = `sqlite:${connection.filePath}`
      await load(connString)
      return await query(
        connString,
        `SELECT name
         FROM sqlite_schema
         WHERE type = '${type}'`,
        [],
      )
    },
  })
  if (objectQuery.status === 'pending') {
    return <div className="p-2">Loading...</div>
  }
  if (objectQuery.status === 'error') {
    return <div className="p-2">Error: {objectQuery.error.message}</div>
  }
  return (
    <SidebarMenuItem>
      <Collapsible className="group/collapsible [&[data-state=open]>button>svg:first-child]:rotate-90">
        <CollapsibleTrigger asChild>
          <SidebarMenuButton>
            <ChevronRightIcon className="transition-transform"/>
            {icon}
            {label}
          </SidebarMenuButton>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <SidebarMenuSub>
            {objectQuery.data.map((subItem, index) => (
              <SidebarMenuButton key={index} title={subItem.name}
                                 onDoubleClick={() => addTab(`Table - ${subItem.name}`, {
                                   type: 'table-view',
                                   connectionId: connection.id,
                                   tableName: subItem.name,
                                 })}>
                {subItem.name}
              </SidebarMenuButton>
            ))}
          </SidebarMenuSub>
        </CollapsibleContent>
      </Collapsible>
    </SidebarMenuItem>
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

function baseName(path: string) {
  return path.replace(/^.+[\/\\]/, '')
}

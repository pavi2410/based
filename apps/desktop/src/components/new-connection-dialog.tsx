import * as React from "react"
import DeviconSqlite from '~icons/devicon/sqlite'
import DeviconMongodb from '~icons/devicon/mongodb'

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
} from "@/components/ui/sidebar"
import { SQLiteConnectionForm } from "./new-connection-forms/sqlite"
import { MongoDBConnectionForm } from "./new-connection-forms/mongodb"

const databaseTypes = [
  { name: "SQLite", icon: DeviconSqlite },
  { name: "MongoDB", icon: DeviconMongodb },
];

export function NewConnectionDialog({
  children,
}: {
  children: React.ReactNode;
}) {
  const [selected, setSelected] = React.useState(databaseTypes[0]);
  return (
    <Dialog>
      <DialogTrigger asChild>
        {children}
      </DialogTrigger>
      <DialogContent className="overflow-hidden p-0 md:max-h-[500px] md:max-w-[700px] lg:max-w-[800px]">
        <SidebarProvider className="items-start">
          <Sidebar collapsible="none" className="hidden md:flex">
            <SidebarHeader>
              <SidebarMenu>
                <SidebarMenuItem className="text-sm text-muted-foreground p-2">
                  Select a database
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarHeader>
            <SidebarContent>
              <SidebarGroup>
                <SidebarGroupContent>
                  <SidebarMenu>
                    {databaseTypes.map((item) => (
                      <SidebarMenuItem key={item.name}>
                        <SidebarMenuButton
                          isActive={item.name === selected.name}
                          onClick={() => setSelected(item)}
                        >
                          <item.icon />
                          <span>{item.name}</span>
                        </SidebarMenuButton>
                      </SidebarMenuItem>
                    ))}
                  </SidebarMenu>
                </SidebarGroupContent>
              </SidebarGroup>
            </SidebarContent>
          </Sidebar>
          <main className="flex h-[480px] flex-1 flex-col overflow-hidden">
            <header className="flex h-16 shrink-0 items-center gap-2 transition-[width,height] ease-linear group-has-[[data-collapsible=icon]]/sidebar-wrapper:h-12">
              <div className="flex flex-col gap-2 p-4">
                <DialogTitle className="xsr-only">Add New Connection</DialogTitle>
                <DialogDescription className="sr-only">
                  Add a new connection to a database.
                </DialogDescription>
              </div>
            </header>
            <div className="flex flex-1 flex-col gap-4 overflow-y-auto p-4 pt-0">
              {selected.name === "SQLite" && <SQLiteConnectionForm />}
              {selected.name === "MongoDB" && <MongoDBConnectionForm />}
            </div>
            <DialogFooter className="px-4">
              <DialogClose asChild>
                <Button type="submit" form="new-connection-form">
                  Add Connection
                </Button>
              </DialogClose>
            </DialogFooter>
          </main>
        </SidebarProvider>
      </DialogContent>
    </Dialog>
  )
}

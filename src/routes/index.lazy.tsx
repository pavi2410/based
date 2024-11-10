import {createLazyFileRoute, Link} from '@tanstack/react-router'
import {Button} from "@/components/ui/button.tsx";
import {toast} from "@/hooks/use-toast.ts";
import {addProject, getProjects, removeProject} from "@/stores.ts";
import {useMutation, useQuery} from "@tanstack/react-query";
import {Loader2Icon, Trash2Icon} from "lucide-react";
import {ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger} from "@/components/ui/context-menu.tsx";
import {
  Dialog, DialogClose,
  DialogContent,
  DialogDescription, DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger
} from "@/components/ui/dialog.tsx";
import {Input} from "@/components/ui/input.tsx";
import {Label} from "@/components/ui/label.tsx";

export const Route = createLazyFileRoute('/')({
  component: Index,
})

function Index() {

  const projectsQuery = useQuery({
    queryKey: ['projects'],
    queryFn: async () => {
      return await getProjects();
    }
  })

  const newProjectMutation = useMutation({
    mutationFn: async (name: string) => {
      await addProject({
        name,
        connections: [],
      })
    },
    onSuccess: async () => {
      toast({
        title: 'Project created',
      });

      await projectsQuery.refetch();
    }
  })

  const deleteProjectMutation = useMutation({
    mutationFn: removeProject,
    onSuccess: async () => {
      await projectsQuery.refetch();
    }
  })

  return (
    <div className="p-2">
      <h3>This is a based app!</h3>

      <Dialog>
        <DialogTrigger asChild>
          <Button>New Project</Button>
        </DialogTrigger>
        <DialogContent className="sm:max-w-[425px]">
          <DialogHeader>
            <DialogTitle>New Project</DialogTitle>
            <DialogDescription>
              Create a new project to start working with databases.
            </DialogDescription>
          </DialogHeader>
          <form
            id="new-project-form"
            onSubmit={(e) => {
              e.preventDefault()
              e.stopPropagation()

              // @ts-ignore
              const name = e.currentTarget.elements['name'].value;
              if (!name || name.trim() === '') return;
              newProjectMutation.mutate(name.trim())
            }}
          >
            <div className="grid gap-4 py-4">
              <div className="grid grid-cols-4 items-center gap-4">
                <Label htmlFor="name" className="text-right text-nowrap">
                  Project Name
                </Label>
                <Input id="name" className="col-span-3"/>
              </div>
            </div>
          </form>
          <DialogFooter>
            <DialogClose asChild>
              <Button type="submit" form="new-project-form">Create Project</Button>
            </DialogClose>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {
        projectsQuery.isLoading && <p>Loading...</p>
      }

      {
        projectsQuery.data && (
          <ul className="flex flex-col gap-2">
            {projectsQuery.data.map((project) => (
              <ContextMenu key={project.name}>
                <ContextMenuTrigger>
                  <Link to="/project/$id" params={{id: project.id}}>
                    <li
                      className="p-4 rounded hover:bg-accent hover:text-accent-foreground"
                    >
                      {project.name}
                    </li>
                  </Link>
                </ContextMenuTrigger>
                <ContextMenuContent>
                  <ContextMenuItem
                    className="!text-red-500"
                    disabled={newProjectMutation.isPending || deleteProjectMutation.isPending}
                    onClick={() => deleteProjectMutation.mutate(project.id)}
                  >
                    {deleteProjectMutation.isPending ? (
                      <Loader2Icon className="animate-spin"/>
                    ) : (
                      <Trash2Icon className="size-4"/>
                    )}
                    &nbsp;
                    Remove
                  </ContextMenuItem>
                </ContextMenuContent>
              </ContextMenu>
            ))}
          </ul>
        )
      }
    </div>
  )
}
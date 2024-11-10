import {createLazyFileRoute} from '@tanstack/react-router'
import {Button} from "@/components/ui/button.tsx";
import {open} from '@tauri-apps/plugin-dialog';
import {toast} from "@/hooks/use-toast.ts";
import {addProject, getProjects, Project, removeProject} from "@/stores.ts";
import {useMutation, useQuery} from "@tanstack/react-query";
import {Loader2Icon, Trash2Icon} from "lucide-react";
import {ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger} from "@/components/ui/context-menu.tsx";

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
    mutationFn: async () => {
      const filePath = await open({
        title: 'Select a SQLite DB file',
        multiple: false,
        directory: false,
        filters: [
          {
            name: 'SQLite DB',
            extensions: ['db', 'sqlite', 'sqlite3']
          }
        ]
      });

      if (!filePath) {
        return {};
      }

      await addProject({
        dbType: 'sqlite',
        filePath,
      })

      return {filePath};
    },
    onSuccess: async ({filePath}) => {
      if (!filePath) {
        toast({title: 'No file selected'});
        return;
      }

      toast({
        title: 'File selected',
        description: filePath,
      });

      await projectsQuery.refetch();
    }
  })

  const deleteProjectMutation = useMutation({
    mutationFn: async (project: Project) => {
      await removeProject(project);
    },
    onSuccess: async () => {
      await projectsQuery.refetch();
    }
  })

  return (
    <div className="p-2">
      <h3>This is a based app!</h3>

      <Button
        disabled={newProjectMutation.isPending || deleteProjectMutation.isPending}
        onClick={() => newProjectMutation.mutate()}
      >
        {newProjectMutation.isPending && <Loader2Icon className="animate-spin"/>}
        Open a SQLite DB file
      </Button>

      {
        projectsQuery.isLoading && <p>Loading...</p>
      }

      {
        projectsQuery.data && (
          <ul className="flex flex-col gap-2">
            {projectsQuery.data.map((project) => (
              <ContextMenu>
                <ContextMenuTrigger>
                  <li
                    key={project.filePath}
                    className="p-4 rounded hover:bg-accent hover:text-accent-foreground"
                    onClick={() => toast({title: 'opening project'})}
                  >
                    <span className="font-bold">{project.dbType}</span>
                    <br />
                    {project.filePath}
                  </li>
                </ContextMenuTrigger>
                <ContextMenuContent>
                  <ContextMenuItem
                    className="!text-red-500"
                    disabled={newProjectMutation.isPending || deleteProjectMutation.isPending}
                    onClick={() => deleteProjectMutation.mutate({
                      dbType: project.dbType,
                      filePath: project.filePath,
                    })}
                  >
                    {deleteProjectMutation.isPending ? (
                      <Loader2Icon className="animate-spin"/>
                    ): (
                      <Trash2Icon className="size-4" />
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
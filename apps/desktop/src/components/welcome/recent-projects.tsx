import { useStore } from "@nanostores/react";
import { Link } from "@tanstack/react-router";
import { formatDistanceToNow } from "date-fns";
import { FolderIcon, XIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  $recentProjects,
  type RecentProject,
  removeRecentProject,
} from "@/stores/project-state";

export function RecentProjects() {
  const recentProjects = useStore($recentProjects);

  if (!recentProjects || recentProjects.length === 0) {
    return null;
  }

  return (
    <div className="space-y-4">
      <h2 className="text-lg font-semibold text-foreground">Recent Projects</h2>
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {recentProjects.map((project) => (
          <ProjectCard key={project.path} project={project} />
        ))}
      </div>
    </div>
  );
}

function ProjectCard({ project }: { project: RecentProject }) {
  const handleRemove = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    removeRecentProject(project.path);
  };

  // Encode project path for URL (Base64)
  const projectId = btoa(project.path);

  return (
    <Link
      to="/project/$projectId"
      params={{ projectId }}
      className="group relative"
    >
      <div className="flex flex-col gap-2 p-4 rounded-lg border bg-muted/50 hover:bg-muted hover:border-primary/50 transition-colors">
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-2">
            <FolderIcon className="size-5 text-muted-foreground" />
            <div className="flex flex-col">
              <span className="font-medium text-sm">{project.name}</span>
              <span className="text-xs text-muted-foreground truncate max-w-[200px]">
                {project.path}
              </span>
            </div>
          </div>
          <Button
            variant="ghost"
            size="icon"
            className="size-6 opacity-0 group-hover:opacity-100 transition-opacity"
            onClick={handleRemove}
            title="Remove from recent"
          >
            <XIcon className="size-4" />
          </Button>
        </div>
        <span className="text-xs text-muted-foreground">
          Opened{" "}
          {formatDistanceToNow(new Date(project.lastOpened), {
            addSuffix: true,
          })}
        </span>
      </div>
    </Link>
  );
}

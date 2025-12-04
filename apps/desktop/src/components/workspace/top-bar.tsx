import { SettingsIcon, RefreshCwIcon, FolderIcon, ChevronDownIcon, HomeIcon, FolderOpenIcon } from "lucide-react";
import { useNavigate } from "@tanstack/react-router";
import { useStore } from "@nanostores/react";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import type { ProjectConfig } from "@/types/project";
import { $recentProjects, type RecentProject } from "@/stores/project-state";
import { useTitlebar } from "@/hooks/use-titlebar";

interface TopBarProps {
  config: ProjectConfig;
  onReloadConfig: () => void;
}

export function TopBar({
  config,
  onReloadConfig,
}: TopBarProps) {
  const navigate = useNavigate();
  const recentProjects = useStore($recentProjects);

  const handleGoHome = () => {
    navigate({ to: "/" });
  };

  const handleSwitchProject = (project: RecentProject) => {
    const projectId = btoa(project.path);
    navigate({ to: "/project/$projectId", params: { projectId } });
  };

  // Filter out current project from recent list
  const otherProjects = recentProjects.filter((p) => p.name !== config.name);

  const { leftRef, rightRef } = useTitlebar([config.name]);

  return (
    <header className="h-10 flex items-center justify-between border-b bg-background/95 backdrop-blur-sm select-none">
      {/* Left: Traffic lights space + Project switcher */}
      <div ref={leftRef} className="flex items-center pl-20 pr-4">
        <Popover>
          <PopoverTrigger asChild>
            <button className="flex items-center gap-1.5 px-2 py-1 rounded hover:bg-muted/50 transition-colors">
              <FolderIcon className="size-3.5 text-muted-foreground" />
              <span className="text-xs font-medium">{config.name}</span>
              <ChevronDownIcon className="size-3 text-muted-foreground" />
            </button>
          </PopoverTrigger>
          <PopoverContent align="start" className="w-56 p-1">
            <button
              onClick={handleGoHome}
              className="flex items-center gap-2 w-full px-2 py-1.5 text-xs rounded hover:bg-muted transition-colors"
            >
              <HomeIcon className="size-3.5 text-muted-foreground" />
              <span>Go to Home</span>
            </button>
            {otherProjects.length > 0 && (
              <>
                <div className="h-px bg-border my-1" />
                <div className="px-2 py-1 text-[10px] text-muted-foreground uppercase tracking-wider">
                  Recent Projects
                </div>
                {otherProjects.slice(0, 5).map((project) => (
                  <button
                    key={project.path}
                    onClick={() => handleSwitchProject(project)}
                    className="flex items-center gap-2 w-full px-2 py-1.5 text-xs rounded hover:bg-muted transition-colors"
                  >
                    <FolderOpenIcon className="size-3.5 text-muted-foreground" />
                    <span className="truncate">{project.name}</span>
                  </button>
                ))}
              </>
            )}
          </PopoverContent>
        </Popover>
      </div>

      {/* Center: Empty space */}
      <div className="flex-1" />

      {/* Right: Actions */}
      <div ref={rightRef} className="flex items-center gap-0.5 px-2">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="size-7"
              onClick={onReloadConfig}
            >
              <RefreshCwIcon className="size-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom">Reload config</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="size-7"
            >
              <SettingsIcon className="size-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom">Settings</TooltipContent>
        </Tooltip>
      </div>
    </header>
  );
}

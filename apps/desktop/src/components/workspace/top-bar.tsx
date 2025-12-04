import { SettingsIcon, RefreshCwIcon, FolderIcon, ChevronDownIcon, HomeIcon, FolderOpenIcon, CircleIcon, UnplugIcon } from "lucide-react";
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
import { $recentProjects, $connection, disconnectConnection, type RecentProject } from "@/stores/project-state";
import { useTitlebar } from "@/hooks/use-titlebar";
import { ConnectionSelector } from "./connection-selector";

function StatusDot({ status }: { status: string }) {
  const colorClass = {
    connected: "fill-emerald-500 text-emerald-500",
    error: "fill-red-500 text-red-500",
    connecting: "fill-amber-500 text-amber-500 animate-pulse",
    disconnected: "fill-muted-foreground/50 text-muted-foreground/50",
  }[status] || "fill-muted-foreground/50 text-muted-foreground/50";

  return <CircleIcon className={`size-1.5 ${colorClass}`} />;
}

function formatConnectionTime(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

interface TopBarProps {
  config: ProjectConfig;
  onReloadConfig: () => void;
  onConnectionChange?: (connKey: string) => void;
  onDisconnect?: () => void;
}

export function TopBar({
  config,
  onReloadConfig,
  onConnectionChange,
  onDisconnect,
}: TopBarProps) {
  const navigate = useNavigate();
  const recentProjects = useStore($recentProjects);
  const connection = useStore($connection);

  const { connKey, status: connectionStatus, stats: connectionStats } = connection;
  const connectionConfig = connKey ? config.connection[connKey] : null;

  const handleGoHome = () => {
    navigate({ to: "/" });
  };

  const handleSwitchProject = (project: RecentProject) => {
    const projectId = btoa(project.path);
    navigate({ to: "/project/$projectId", params: { projectId } });
  };

  // Filter out current project from recent list
  const otherProjects = recentProjects.filter((p) => p.name !== config.name);

  const { leftRef, rightRef } = useTitlebar([config.name, connKey]);

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
        {/* Connection controls */}
        {connKey && connectionConfig && (
          <>
            <Tooltip>
              <TooltipTrigger asChild>
                <div className="flex items-center gap-1.5 px-1 rounded cursor-default">
                  <StatusDot status={connectionStatus} />
                </div>
              </TooltipTrigger>
              {connectionStatus === "connected" && connectionStats && (
                <TooltipContent side="bottom" className="text-xs">
                  <div className="space-y-0.5">
                    <div>Connected in {formatConnectionTime(connectionStats.connectionTimeMs)}</div>
                    <div className="text-muted-foreground">
                      {new Date(connectionStats.connectedAt).toLocaleTimeString()}
                    </div>
                  </div>
                </TooltipContent>
              )}
            </Tooltip>
            <ConnectionSelector
              connections={config.connection}
              connKey={connKey}
              onConnectionChange={(key) => onConnectionChange?.(key)}
              compact
            />
            <Popover>
              <Tooltip>
                <TooltipTrigger asChild>
                  <PopoverTrigger asChild>
                    <button className="p-1 rounded hover:bg-muted/50 text-muted-foreground hover:text-foreground transition-colors">
                      <UnplugIcon className="size-3" />
                    </button>
                  </PopoverTrigger>
                </TooltipTrigger>
                <TooltipContent side="bottom">Disconnect</TooltipContent>
              </Tooltip>
              <PopoverContent side="bottom" className="w-auto p-2">
                <div className="flex flex-col gap-2 justify-end">
                  <p className="text-xs max-w-3xs">Are you sure you want to disconnect from <b>{connectionConfig?.label || connKey}</b>?</p>
                  <Button
                    variant="destructive"
                    size="sm"
                    className="h-7 text-xs w-fit ml-auto"
                    onClick={async () => {
                      await disconnectConnection();
                      onDisconnect?.();
                    }}
                  >
                    Yes
                  </Button>
                </div>
              </PopoverContent>
            </Popover>
          </>
        )}
      </div>

      {/* Center: Empty space (drag region) */}
      <div className="flex-1" />

      {/* Right: Group badge + Actions */}
      <div ref={rightRef} className="flex items-center gap-1 px-2">
        {connectionConfig?.group && (
          <span className="text-[10px] text-muted-foreground/70 capitalize px-1.5 py-0.5 rounded bg-muted/30 mr-1">
            {connectionConfig.group}
          </span>
        )}
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

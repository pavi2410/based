import { SettingsIcon, RefreshCwIcon, FolderOpenIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import type { ProjectConfig } from "@/types/project";
import { ConnectionSelector } from "./connection-selector";

interface TopBarProps {
  config: ProjectConfig;
  projectPath: string;
  activeConnection: string | null;
  onConnectionChange: (connKey: string) => void;
  onReloadConfig: () => void;
}

export function TopBar({
  config,
  projectPath,
  activeConnection,
  onConnectionChange,
  onReloadConfig,
}: TopBarProps) {
  return (
    <div className="border-b bg-background">
      <div className="flex items-center justify-between px-4 py-2">
        {/* Left: Project Info */}
        <div className="flex items-center gap-4">
          <div className="flex flex-col">
            <div className="flex items-center gap-2">
              <FolderOpenIcon className="size-4 text-muted-foreground" />
              <h1 className="text-sm font-semibold">{config.name}</h1>
              <Badge variant="outline" className="text-xs">
                v{config.version}
              </Badge>
            </div>
            <p className="text-xs text-muted-foreground truncate max-w-md">
              {projectPath}
            </p>
          </div>
        </div>

        {/* Center: Connection Selector */}
        <div className="flex items-center gap-2">
          <ConnectionSelector
            connections={config.connection}
            activeConnection={activeConnection}
            onConnectionChange={onConnectionChange}
          />
        </div>

        {/* Right: Actions */}
        <div className="flex items-center gap-2">
          <Button
            variant="ghost"
            size="sm"
            onClick={onReloadConfig}
            title="Reload configuration"
          >
            <RefreshCwIcon className="size-4" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            title="Project settings"
          >
            <SettingsIcon className="size-4" />
          </Button>
        </div>
      </div>

      {/* Bottom: Description */}
      {config.description && (
        <div className="px-4 pb-2">
          <p className="text-xs text-muted-foreground">{config.description}</p>
        </div>
      )}
    </div>
  );
}

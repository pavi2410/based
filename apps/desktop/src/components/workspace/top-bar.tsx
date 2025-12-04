import { SettingsIcon, RefreshCwIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { ProjectConfig } from "@/types/project";

interface TopBarProps {
  config: ProjectConfig;
  onReloadConfig: () => void;
}

export function TopBar({
  config,
  onReloadConfig,
}: TopBarProps) {
  return (
    <header
      data-tauri-drag-region
      className="h-12 flex items-center justify-between border-b bg-background/80 backdrop-blur-sm select-none"
    >
      {/* Left: Traffic lights space + Project name */}
      <div className="flex items-center gap-3 pl-20 pr-4">
        <span className="text-sm font-medium">{config.name}</span>
      </div>

      {/* Center: Empty draggable space */}
      <div className="flex-1" />

      {/* Right: Actions */}
      <div className="flex items-center gap-1 px-3">
        <Button
          variant="ghost"
          size="icon"
          className="size-8"
          onClick={onReloadConfig}
          title="Reload configuration"
        >
          <RefreshCwIcon className="size-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="size-8"
          title="Project settings"
        >
          <SettingsIcon className="size-4" />
        </Button>
      </div>
    </header>
  );
}

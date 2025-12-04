import { CircleIcon, UnplugIcon } from "lucide-react";
import { useStore } from "@nanostores/react";
import { Tooltip, TooltipTrigger, TooltipContent } from "@/components/ui/tooltip";
import { Popover, PopoverTrigger, PopoverContent } from "@/components/ui/popover";
import {
  $connection,
  $projectConfig,
  disconnectConnection,
} from "@/stores/project-state";
import { Button } from "@/components/ui/button";
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

interface StatusBarProps {
  onDisconnect?: () => void;
  onConnectionChange?: (connKey: string) => void;
}

export function StatusBar({ onDisconnect, onConnectionChange }: StatusBarProps = {}) {
  const connection = useStore($connection);
  const projectConfig = useStore($projectConfig);

  const { connKey, status: connectionStatus, stats: connectionStats } = connection;
  const connectionConfig = connKey && projectConfig
    ? projectConfig.connection[connKey]
    : null;

  return (
    <div className="h-7 border-t bg-muted/20 px-2 flex items-center justify-between text-[11px]">
      {/* Left: Status + Connection */}
      <div className="flex items-center gap-2">
        <Tooltip>
          <TooltipTrigger asChild>
            <div className="flex items-center gap-1.5 px-1 rounded hover:bg-muted/50 transition-colors cursor-default">
              <StatusDot status={connectionStatus} />
              <span className="text-muted-foreground capitalize">{connectionStatus}</span>
            </div>
          </TooltipTrigger>
          {connectionStatus === "connected" && connectionStats && (
            <TooltipContent side="top" className="text-xs">
              <div className="space-y-0.5">
                <div>Connected in {formatConnectionTime(connectionStats.connectionTimeMs)}</div>
                <div className="text-muted-foreground">
                  {new Date(connectionStats.connectedAt).toLocaleTimeString()}
                </div>
              </div>
            </TooltipContent>
          )}
        </Tooltip>

        {connKey && connectionConfig && projectConfig && (
          <>
            <span className="text-border">•</span>
            <ConnectionSelector
              connections={projectConfig.connection}
              connKey={connKey}
              onConnectionChange={(key) => onConnectionChange?.(key)}
              compact
            />
            <Popover>
              <Tooltip>
                <TooltipTrigger asChild>
                  <PopoverTrigger asChild>
                    <button className="p-0.5 rounded hover:bg-muted/50 text-muted-foreground hover:text-foreground transition-colors">
                      <UnplugIcon className="size-3" />
                    </button>
                  </PopoverTrigger>
                </TooltipTrigger>
                <TooltipContent side="top">Disconnect</TooltipContent>
              </Tooltip>
              <PopoverContent side="top" className="w-auto p-2">
                <div className="flex flex-col gap-2">
                  <p className="text-xs">Disconnect?</p>
                  <Button
                    variant="destructive"
                    size="sm"
                    className="h-7 text-xs"
                    onClick={async () => {
                      await disconnectConnection();
                      onDisconnect?.();
                    }}
                  >
                    Disconnect
                  </Button>
                </div>
              </PopoverContent>
            </Popover>
          </>
        )}
      </div>

      {/* Right: Group badge */}
      {connectionConfig?.group && (
        <span className="text-muted-foreground/70 capitalize px-1.5 py-0.5 rounded bg-muted/30">
          {connectionConfig.group}
        </span>
      )}
    </div>
  );
}

import { CircleCheckIcon, CircleXIcon, CircleDotIcon, UnplugIcon } from "lucide-react";
import { useStore } from "@nanostores/react";
import DeviconSqlite from "~icons/devicon/sqlite";
import DeviconMongodb from "~icons/devicon/mongodb";
import DeviconPostgresql from "~icons/devicon/postgresql";
import { Tooltip, TooltipTrigger, TooltipContent } from "@/components/ui/tooltip";
import { Popover, PopoverTrigger, PopoverContent } from "@/components/ui/popover";
import { $connectionStats, disconnectConnection } from "@/stores/project-state";
import { Button } from "@/components/ui/button";
import type { ConnectionConfig } from "@/types/project";

interface StatusBarProps {
  activeConnection: string | null;
  connectionConfig: ConnectionConfig | null;
  connectionStatus: "connected" | "disconnected" | "connecting" | "error";
}

function getStatusIcon(status: string) {
  switch (status) {
    case "connected":
      return <CircleCheckIcon className="size-3 text-green-500" />;
    case "error":
      return <CircleXIcon className="size-3 text-destructive" />;
    case "connecting":
      return <CircleDotIcon className="size-3 text-yellow-500 animate-pulse" />;
    default:
      return <CircleDotIcon className="size-3 text-muted-foreground" />;
  }
}

function getStatusLabel(status: string) {
  switch (status) {
    case "connected":
      return "Connected";
    case "error":
      return "Error";
    case "connecting":
      return "Connecting...";
    default:
      return "Disconnected";
  }
}

function getEngineIcon(engine: string) {
  switch (engine) {
    case "sqlite":
      return <DeviconSqlite className="size-4" />;
    case "mongodb":
      return <DeviconMongodb className="size-4" />;
    case "postgres":
      return <DeviconPostgresql className="size-4" />;
    default:
      return null;
  }
}

function formatConnectionTime(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
}

export function StatusBar({
  activeConnection,
  connectionConfig,
  connectionStatus,
}: StatusBarProps) {
  const connectionStats = useStore($connectionStats);

  const statusContent = (
    <div className="flex items-center gap-2">
      {getStatusIcon(connectionStatus)}
      <span className="text-muted-foreground">{getStatusLabel(connectionStatus)}</span>
    </div>
  );

  return (
    <div className="border-t bg-muted/30 px-4 py-1.5 flex items-center justify-between text-xs">
      {/* Left: Connection Status */}
      <div className="flex items-center gap-4">
        {connectionStatus === "connected" && connectionStats ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <button className="flex items-center gap-2 hover:bg-muted/50 rounded px-1 -mx-1 transition-colors">
                {getStatusIcon(connectionStatus)}
                <span className="text-muted-foreground">{getStatusLabel(connectionStatus)}</span>
              </button>
            </TooltipTrigger>
            <TooltipContent side="top" className="space-y-1">
              <div className="flex items-center justify-between gap-4">
                <span className="text-muted-foreground">Connection time:</span>
                <span className="font-medium">{formatConnectionTime(connectionStats.connectionTimeMs)}</span>
              </div>
              <div className="flex items-center justify-between gap-4">
                <span className="text-muted-foreground">Connected at:</span>
                <span className="font-medium">
                  {new Date(connectionStats.connectedAt).toLocaleTimeString()}
                </span>
              </div>
            </TooltipContent>
          </Tooltip>
        ) : (
          statusContent
        )}

        {activeConnection && connectionConfig && (
          <>
            <div className="flex items-center gap-2">
              <span className="text-muted-foreground">|</span>
              {getEngineIcon(connectionConfig.engine)}
              <span className="font-medium">{connectionConfig.label || activeConnection}</span>
            </div>
            <Popover>
              <Tooltip>
                <TooltipTrigger asChild>
                  <PopoverTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-5"
                    >
                      <UnplugIcon className="size-3" />
                    </Button>
                  </PopoverTrigger>
                </TooltipTrigger>
                <TooltipContent side="top">Disconnect</TooltipContent>
              </Tooltip>
              <PopoverContent side="top" className="w-auto p-3">
                <div className="flex flex-col gap-2">
                  <p className="text-sm">Disconnect from this database?</p>
                  <div className="flex justify-end gap-2">
                    <Button
                      variant="destructive"
                      size="sm"
                      onClick={disconnectConnection}
                    >
                      Disconnect
                    </Button>
                  </div>
                </div>
              </PopoverContent>
            </Popover>
          </>
        )}
      </div>

      {/* Right: Additional Info */}
      <div className="flex items-center gap-2 text-muted-foreground">
        {connectionConfig?.group && (
          <span className="capitalize">{connectionConfig.group}</span>
        )}
      </div>
    </div>
  );
}

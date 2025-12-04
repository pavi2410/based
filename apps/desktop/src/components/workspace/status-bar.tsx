import { CircleCheckIcon, CircleXIcon, CircleDotIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
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

export function StatusBar({
  activeConnection,
  connectionConfig,
  connectionStatus,
}: StatusBarProps) {
  return (
    <div className="border-t bg-muted/30 px-4 py-1.5 flex items-center justify-between text-xs">
      {/* Left: Connection Status */}
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-2">
          {getStatusIcon(connectionStatus)}
          <span className="text-muted-foreground">{getStatusLabel(connectionStatus)}</span>
        </div>

        {activeConnection && connectionConfig && (
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground">|</span>
            <Badge variant="secondary" className="text-xs font-normal">
              {connectionConfig.engine.toUpperCase()}
            </Badge>
            <span className="font-medium">{connectionConfig.label || activeConnection}</span>
          </div>
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

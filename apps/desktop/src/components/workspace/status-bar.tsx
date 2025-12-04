import { CircleCheckIcon, CircleXIcon, CircleDotIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import type { DatabaseConfig } from "@/types/project";

interface StatusBarProps {
  activeDatabase: string | null;
  databaseConfig: DatabaseConfig | null;
  activeEnvironment: string;
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
  activeDatabase,
  databaseConfig,
  activeEnvironment,
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

        {activeDatabase && databaseConfig && (
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground">|</span>
            <Badge variant="secondary" className="text-xs font-normal">
              {databaseConfig.type.toUpperCase()}
            </Badge>
            <span className="font-medium">{databaseConfig.name}</span>
          </div>
        )}
      </div>

      {/* Right: Environment */}
      <div className="flex items-center gap-2">
        <span className="text-muted-foreground">Environment:</span>
        <Badge
          variant={activeEnvironment === "production" ? "destructive" : "outline"}
          className="text-xs font-normal capitalize"
        >
          {activeEnvironment}
        </Badge>
      </div>
    </div>
  );
}

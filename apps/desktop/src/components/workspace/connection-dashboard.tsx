import { useMemo } from "react";
import { DatabaseIcon, HardDriveIcon, CloudIcon, Loader2Icon } from "lucide-react";
import { useStore } from "@nanostores/react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import type { ProjectConfig, ConnectionConfig, Engine } from "@/types/project";
import { switchConnection, $connectionStatus } from "@/stores/project-state";

interface ConnectionDashboardProps {
  config: ProjectConfig;
}

const ENGINE_INFO: Record<Engine, { icon: React.ReactNode; label: string; color: string }> = {
  sqlite: {
    icon: <DatabaseIcon className="size-6" />,
    label: "SQLite",
    color: "text-blue-500",
  },
  postgres: {
    icon: <DatabaseIcon className="size-6" />,
    label: "PostgreSQL",
    color: "text-sky-500",
  },
  mongodb: {
    icon: <DatabaseIcon className="size-6" />,
    label: "MongoDB",
    color: "text-green-500",
  },
};

const GROUP_INFO: Record<string, { icon: React.ReactNode; label: string }> = {
  local: {
    icon: <HardDriveIcon className="size-4" />,
    label: "Local",
  },
  remote: {
    icon: <CloudIcon className="size-4" />,
    label: "Remote",
  },
};

interface ConnectionCardProps {
  connKey: string;
  config: ConnectionConfig;
  onConnect: (connKey: string) => void;
  isConnecting: boolean;
}

function ConnectionCard({ connKey, config, onConnect, isConnecting }: ConnectionCardProps) {
  const engineInfo = ENGINE_INFO[config.engine];
  
  return (
    <Card className="group hover:border-primary/50 transition-colors">
      <CardHeader className="pb-3">
        <div className="flex items-start justify-between">
          <div className={`${engineInfo.color}`}>
            {engineInfo.icon}
          </div>
          <Badge variant="secondary" className="text-xs">
            {engineInfo.label}
          </Badge>
        </div>
        <CardTitle className="text-base mt-2">
          {config.label || connKey}
        </CardTitle>
        <CardDescription className="text-xs font-mono">
          {connKey}
        </CardDescription>
      </CardHeader>
      <CardContent className="pt-0">
        <Button
          className="w-full"
          onClick={() => onConnect(connKey)}
          disabled={isConnecting || config.disabled}
        >
          {isConnecting ? (
            <>
              <Loader2Icon className="size-4 mr-2 animate-spin" />
              Connecting...
            </>
          ) : (
            "Connect"
          )}
        </Button>
      </CardContent>
    </Card>
  );
}

interface ConnectionGroupProps {
  groupKey: string;
  connections: Array<{ key: string; config: ConnectionConfig }>;
  onConnect: (connKey: string) => void;
  connectingKey: string | null;
}

function ConnectionGroup({ groupKey, connections, onConnect, connectingKey }: ConnectionGroupProps) {
  const groupInfo = GROUP_INFO[groupKey] || {
    icon: <DatabaseIcon className="size-4" />,
    label: groupKey.charAt(0).toUpperCase() + groupKey.slice(1),
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        {groupInfo.icon}
        <span className="font-medium">{groupInfo.label}</span>
        <span className="text-xs">({connections.length})</span>
      </div>
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
        {connections.map(({ key, config }) => (
          <ConnectionCard
            key={key}
            connKey={key}
            config={config}
            onConnect={onConnect}
            isConnecting={connectingKey === key}
          />
        ))}
      </div>
    </div>
  );
}

export function ConnectionDashboard({ config }: ConnectionDashboardProps) {
  const connectionStatus = useStore($connectionStatus);
  
  // Track which connection is being connected
  const isConnecting = connectionStatus === "connecting";

  // Group connections by their group field
  const groupedConnections = useMemo(() => {
    const groups: Record<string, Array<{ key: string; config: ConnectionConfig }>> = {};
    
    for (const [key, connConfig] of Object.entries(config.connection)) {
      const group = connConfig.group || "other";
      if (!groups[group]) {
        groups[group] = [];
      }
      groups[group].push({ key, config: connConfig });
    }

    // Sort connections within each group by order, then by label/key
    for (const group of Object.values(groups)) {
      group.sort((a, b) => {
        const orderA = a.config.order ?? 999;
        const orderB = b.config.order ?? 999;
        if (orderA !== orderB) return orderA - orderB;
        const labelA = a.config.label || a.key;
        const labelB = b.config.label || b.key;
        return labelA.localeCompare(labelB);
      });
    }

    // Sort groups: local first, then remote, then others alphabetically
    const groupOrder = ["local", "remote"];
    const sortedGroups = Object.entries(groups).sort(([a], [b]) => {
      const indexA = groupOrder.indexOf(a);
      const indexB = groupOrder.indexOf(b);
      if (indexA !== -1 && indexB !== -1) return indexA - indexB;
      if (indexA !== -1) return -1;
      if (indexB !== -1) return 1;
      return a.localeCompare(b);
    });

    return sortedGroups;
  }, [config.connection]);

  const handleConnect = async (connKey: string) => {
    await switchConnection(connKey);
  };

  const totalConnections = Object.keys(config.connection).length;

  return (
    <div className="flex flex-col items-center justify-center min-h-full p-8">
      <div className="w-full max-w-4xl space-y-8">
        {/* Header */}
        <div className="text-center space-y-2">
          <h1 className="text-2xl font-bold">{config.name}</h1>
          <p className="text-muted-foreground">
            Select a database connection to get started
          </p>
          <p className="text-sm text-muted-foreground">
            {totalConnections} connection{totalConnections !== 1 ? "s" : ""} configured
          </p>
        </div>

        {/* Connection Groups */}
        <div className="space-y-8">
          {groupedConnections.map(([groupKey, connections]) => (
            <ConnectionGroup
              key={groupKey}
              groupKey={groupKey}
              connections={connections}
              onConnect={handleConnect}
              connectingKey={isConnecting ? null : null} // TODO: track specific key
            />
          ))}
        </div>

        {/* Empty state if no connections */}
        {totalConnections === 0 && (
          <div className="text-center py-12 border-2 border-dashed rounded-lg">
            <DatabaseIcon className="size-12 mx-auto text-muted-foreground mb-4" />
            <h3 className="font-medium mb-2">No connections configured</h3>
            <p className="text-sm text-muted-foreground">
              Add connections to your <code className="bg-muted px-1 rounded">.based/config.toml</code> file
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

import { useMemo } from "react";
import { DatabaseIcon, HardDriveIcon, CloudIcon, Loader2Icon, ChevronRightIcon } from "lucide-react";
import DeviconSqlite from "~icons/devicon/sqlite";
import DeviconMongodb from "~icons/devicon/mongodb";
import DeviconPostgresql from "~icons/devicon/postgresql";
import type { ProjectConfig, ConnectionConfig, Engine } from "@/types/project";

interface ConnectionDashboardProps {
  config: ProjectConfig;
  onConnect: (connKey: string) => void;
}

const ENGINE_ICONS: Record<Engine, React.ReactNode> = {
  sqlite: <DeviconSqlite className="size-6" />,
  postgres: <DeviconPostgresql className="size-6" />,
  mongodb: <DeviconMongodb className="size-6" />,
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

interface ConnectionRowProps {
  connKey: string;
  config: ConnectionConfig;
  onConnect: (connKey: string) => void;
  isConnecting: boolean;
}

function ConnectionRow({ connKey, config, onConnect, isConnecting }: ConnectionRowProps) {
  const engineIcon = ENGINE_ICONS[config.engine] || <DatabaseIcon className="size-6" />;

  return (
    <button
      type="button"
      className="w-full flex items-center gap-4 px-4 py-3 rounded-lg border bg-muted/50 hover:bg-muted hover:border-primary/50 transition-colors text-left disabled:opacity-50 disabled:cursor-not-allowed"
      onClick={() => onConnect(connKey)}
      disabled={isConnecting || config.disabled === true}
    >
      {engineIcon}
      <div className="flex-1 min-w-0">
        <div className="font-medium truncate">
          {config.label || connKey}
        </div>
        <div className="text-xs text-muted-foreground font-mono truncate">
          {connKey}
        </div>
      </div>
      {isConnecting ? (
        <Loader2Icon className="size-4 animate-spin text-muted-foreground shrink-0" />
      ) : (
        <ChevronRightIcon className="size-4 text-muted-foreground shrink-0" />
      )}
    </button>
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
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-2">
        {connections.map(({ key, config }) => (
          <ConnectionRow
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

export function ConnectionDashboard({ config, onConnect }: ConnectionDashboardProps) {

  // Group connections by their group field
  const groupedConnections = useMemo(() => {
    const groups: Record<string, Array<{ key: string; config: ConnectionConfig }>> = {};
    
    for (const [key, connConfig] of Object.entries(config.connection)) {
      if (!connConfig) continue;
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

  const handleConnect = (connKey: string) => {
    onConnect(connKey);
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
              connectingKey={null} // TODO: track specific key
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

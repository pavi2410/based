import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useContext } from "react";
import { ConnectionDashboard } from "@/components/workspace/connection-dashboard";
import { ProjectContext } from "../project.$projectId";

export const Route = createFileRoute("/project/$projectId/")({
  component: ProjectDashboard,
});

function ProjectDashboard() {
  const ctx = useContext(ProjectContext);
  const navigate = useNavigate();

  // Context may not be ready during route transitions
  if (!ctx) {
    return null;
  }

  const { config, projectId, projectPath, reloadConfig } = ctx;

  const handleConnect = (connKey: string) => {
    navigate({
      to: "/project/$projectId/conn/$connKey",
      params: { projectId, connKey },
    });
  };

  const handleConnectionAdded = (connKey: string) => {
    reloadConfig();
    handleConnect(connKey);
  };

  return (
    <ConnectionDashboard
      config={config}
      projectPath={projectPath}
      onConnect={handleConnect}
      onConnectionAdded={handleConnectionAdded}
    />
  );
}

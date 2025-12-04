import { createFileRoute } from "@tanstack/react-router";
import { Button } from "@/components/ui/button.tsx";
import { StarIcon, Loader2Icon } from "lucide-react";
import { RecentProjects } from "@/components/welcome/recent-projects";
import { ActionButtons } from "@/components/welcome/action-buttons";
import { useCliArgs } from "@/hooks/use-cli-args";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

export const Route = createFileRoute("/")({
  component: Index,
});

function Index() {
  const { isProcessing, error } = useCliArgs();

  if (isProcessing) {
    return (
      <div className="flex items-center justify-center h-screen">
        <div className="flex flex-col items-center gap-4">
          <Loader2Icon className="size-8 animate-spin text-muted-foreground" />
          <p className="text-sm text-muted-foreground">Processing CLI arguments...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-screen overflow-hidden">
      {/* Header */}
      <div className="flex justify-between items-center border-b p-4">
        <Branding />
      </div>

      {/* CLI Error Alert */}
      {error && (
        <div className="p-4">
          <Alert variant="destructive">
            <AlertTitle>Failed to open project from CLI</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        </div>
      )}

      {/* Welcome Content */}
      <div className="flex-1 flex flex-col items-center justify-center p-8 overflow-y-auto">
        <div className="max-w-4xl w-full space-y-12">
          {/* Hero Section */}
          <div className="text-center space-y-6">
            <div>
              <h1 className="text-4xl font-bold text-foreground mb-2">
                Welcome to Based
              </h1>
              <p className="text-lg text-muted-foreground">
                Git-Friendly Database Client for Developers
              </p>
            </div>
            <ActionButtons />
          </div>

          {/* Recent Projects */}
          <RecentProjects />

          {/* Getting Started */}
          <div className="space-y-4">
            <h2 className="text-lg font-semibold text-foreground">Getting Started</h2>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div className="p-4 rounded-lg border">
                <h3 className="font-medium mb-2">Open a Project</h3>
                <p className="text-sm text-muted-foreground">
                  Open an existing folder with a <code className="px-1 py-0.5 rounded bg-muted">.based/</code> directory,
                  or initialize a new project in any folder.
                </p>
              </div>
              <div className="p-4 rounded-lg border">
                <h3 className="font-medium mb-2">Version Control</h3>
                <p className="text-sm text-muted-foreground">
                  All your database configs and queries are stored as plain text files,
                  making them perfect for git and team collaboration.
                </p>
              </div>
              <div className="p-4 rounded-lg border">
                <h3 className="font-medium mb-2">Multi-Database Support</h3>
                <p className="text-sm text-muted-foreground">
                  Connect to SQLite, MongoDB, and PostgreSQL databases within a single project,
                  with environment-specific configurations.
                </p>
              </div>
              <div className="p-4 rounded-lg border">
                <h3 className="font-medium mb-2">Saved Queries</h3>
                <p className="text-sm text-muted-foreground">
                  Save and organize your queries as <code className="px-1 py-0.5 rounded bg-muted">.sqlx</code> and{" "}
                  <code className="px-1 py-0.5 rounded bg-muted">.mongox</code> files with YAML metadata.
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function Branding() {
  return (
    <div className="flex flex-row items-center">
      <div className="mr-4">
        <h1 className="text-sm font-medium">
          <span className="text-muted-foreground">pavi2410 / </span>
          <span className="text-foreground font-bold">based</span>
        </h1>
        <em className="text-xs text-muted-foreground block">Git-Friendly Database Client</em>
      </div>
      <div className="flex items-center gap-3 border-l pl-4">
        <Button
          className="shadow-none text-muted-foreground hover:text-primary hover:bg-primary/10"
          variant="outline"
          size="sm"
          asChild
        >
          <a
            href="https://github.com/pavi2410/based"
            target="_blank"
            rel="noreferrer"
          >
            <StarIcon className="mr-1 h-3.5 w-3.5" />
            Star on GitHub
          </a>
        </Button>
      </div>
    </div>
  );
}

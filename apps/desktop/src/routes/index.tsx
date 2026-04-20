import { createFileRoute } from "@tanstack/react-router";
import { Button } from "@/components/ui/button.tsx";
import { StarIcon } from "lucide-react";
import { RecentProjects } from "@/components/welcome/recent-projects";
import { ActionButtons } from "@/components/welcome/action-buttons";
import { useTitlebar } from "@/hooks/use-titlebar";

export const Route = createFileRoute("/")({
  component: Index,
});

function Index() {
  const { leftRef, rightRef } = useTitlebar();

  return (
    <div className="flex flex-col h-screen overflow-hidden">
      {/* Header */}
      <header className="h-10 flex items-center justify-between border-b bg-background/95 backdrop-blur-sm select-none">
        <div ref={leftRef} className="pl-20 pr-4">
          <Branding />
        </div>
        <div className="flex-1" />
        <div ref={rightRef} className="pr-4">
          <Button
            className="shadow-none text-muted-foreground hover:text-foreground hover:bg-yellow-500/10 group"
            variant="outline"
            size="sm"
            asChild
          >
            <a
              href="https://github.com/pavi2410/based"
              target="_blank"
              rel="noreferrer"
            >
              <StarIcon className="mr-1 h-3.5 w-3.5 group-hover:text-yellow-400 group-hover:fill-yellow-400 transition-colors" />
              Star on GitHub
            </a>
          </Button>
        </div>
      </header>

      {/* Main Content */}
      <div className="flex-1 flex flex-col items-center justify-center p-8 overflow-y-auto">
        <div className="max-w-3xl w-full space-y-8">
          {/* Action Buttons */}
          <div className="flex justify-center">
            <ActionButtons />
          </div>

          {/* Recent Projects */}
          <RecentProjects />
        </div>
      </div>
    </div>
  );
}

function Branding() {
  return (
    <div className="flex flex-row items-center">
      <div>
        <h1 className="text-sm font-medium">
          <span className="text-muted-foreground">pavi2410 / </span>
          <span className="text-foreground font-bold">based</span>
        </h1>
        <em className="text-xs text-muted-foreground block">
          Git-Friendly Database Client
        </em>
      </div>
    </div>
  );
}

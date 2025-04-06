import { Button } from "@/components/ui/button.tsx";
import { 
  MongoDBExplorer, 
  SQLiteExplorer 
} from "@/components/database-explorers";
import { useConnection } from "@/queries/use-connection";
import { Link, createFileRoute } from "@tanstack/react-router";
import { Loader2Icon, RefreshCcwIcon } from "lucide-react";

export const Route = createFileRoute("/conn/$id")({
  component: RouteComponent,
});

function RouteComponent() {
  const { id } = Route.useParams();
  
  const { status, retry } = useConnection(id);

  // Using exhaustive switch pattern for better type safety
  switch (status.status) {
    case 'loading':
      return (
        <div className="flex flex-col items-center justify-center h-full gap-4">
          <Loader2Icon className="animate-spin" />
          Loading...
        </div>
      );
    case 'error':
      return (
        <div className="flex flex-col items-center justify-center h-full gap-4 p-4">
          <div className="text-destructive text-lg font-medium">Connection Error</div>
          <div className="text-destructive/80 text-center max-w-md">
            {status.error.message}
          </div>
          <div className="flex gap-4 mt-4">
            <Button 
              variant="outline" 
              onClick={retry} 
              className="flex items-center gap-2"
            >
              <RefreshCcwIcon className="size-4" />
              Retry Connection
            </Button>
            <Button asChild>
              <Link to="/">
                Go Home
              </Link>
            </Button>
          </div>
        </div>
      );
    case 'success':
      const connMeta = status.data;
      
      // Route to the appropriate explorer based on database type
      if (connMeta.dbType === "mongodb") {
        return <MongoDBExplorer connMeta={connMeta} />;
      } else {
        // Default to SQLite
        return <SQLiteExplorer connMeta={connMeta} />;
      }
  }
}

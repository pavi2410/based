import { Input } from "@/components/ui/input.tsx";
import { Label } from "@/components/ui/label.tsx";
import { newConnectionMutation } from "@/mutations/new-connection";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { AlertCircle, InfoIcon } from "lucide-react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useState } from "react";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { PasswordInput } from "@/components/password-input";

export function MongoDBConnectionForm() {
  const newConnMutation = newConnectionMutation();
  const [connectionTab, setConnectionTab] = useState<string>("form");
  const [formError, setFormError] = useState<string | null>(null);

  const handleFormSubmit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    e.stopPropagation();
    setFormError(null);

    const formData = new FormData(e.currentTarget);

    if (connectionTab === "connectionString") {
      // Handle direct connection string input
      const connectionString = formData.get("connectionString") as string;

      if (!connectionString) {
        return;
      }

      // Validate that the connection string includes a database name
      if (!connectionString.includes('/') || connectionString.endsWith('/')) {
        setFormError("Connection string must include a database name (e.g., mongodb://hostname:port/database_name)");
        return;
      }

      // Validate that the connection string starts with mongodb:// or mongodb+srv://
      if (!connectionString.startsWith('mongodb://') && !connectionString.startsWith('mongodb+srv://')) {
        setFormError("Connection string must start with 'mongodb://' or 'mongodb+srv://'");
        return;
      }

      newConnMutation.mutate({
        dbType: "mongodb",
        connectionString,
        tags: [],
      });
    } else {
      // Handle form-based connection
      const host = formData.get("host") as string || "localhost";
      const port = formData.get("port") as string || "27017";
      const database = formData.get("database") as string;
      const username = formData.get("username") as string;
      const password = formData.get("password") as string;
      const authSource = formData.get("authSource") as string;
      
      // Validate that database name is provided
      if (!database) {
        setFormError("Database name is required");
        return;
      }
      
      // Build the MongoDB connection string
      let connectionString = "mongodb://";
      
      // Add credentials if provided
      if (username) {
        connectionString += username;
        if (password) {
          connectionString += `:${password}`;
        }
        connectionString += '@';
      }
      
      // Add host and port
      connectionString += `${host}:${port}`;
      
      // Add database name
      connectionString += `/${database}`;
      
      // Add auth source if provided
      if (authSource && username) {
        connectionString += `?authSource=${authSource}`;
      } else if (username) {
        // Default to admin authSource to prevent SCRAM authentication failures
        connectionString += `?authSource=admin`;
      }
      
      newConnMutation.mutate({
        dbType: "mongodb",
        connectionString,
        tags: [],
      });
    }
  };

  return (
    <form
      id="new-connection-form"
      onSubmit={handleFormSubmit}
    >
      <Tabs value={connectionTab} onValueChange={setConnectionTab} className="w-full">
        <TabsList className="grid w-full grid-cols-2 mb-4 bg-muted-foreground/10">
          <TabsTrigger value="form">Connection Form</TabsTrigger>
          <TabsTrigger value="connectionString">Connection String</TabsTrigger>
        </TabsList>

        <TabsContent value="form">
          <div className="grid gap-4 py-2">
            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="host" className="text-right text-nowrap">
                Host
              </Label>
              <div className="col-span-3">
                <Input id="host" name="host" defaultValue="localhost" placeholder="localhost" />
              </div>
            </div>

            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="port" className="text-right text-nowrap">
                Port
              </Label>
              <div className="col-span-3">
                <Input id="port" name="port" defaultValue="27017" placeholder="27017" />
              </div>
            </div>

            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="database" className="text-right text-nowrap">
                Database
              </Label>
              <div className="col-span-3">
                <Input id="database" name="database" placeholder="my_database" required />
              </div>
            </div>

            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="username" className="text-right text-nowrap">
                Username
                <TooltipProvider>
                  <Tooltip>
                    <TooltipTrigger>
                      <InfoIcon className="h-3 w-3 ml-1 inline" />
                    </TooltipTrigger>
                    <TooltipContent>
                      <p className="max-w-xs">If authentication is required, enter your username</p>
                    </TooltipContent>
                  </Tooltip>
                </TooltipProvider>
              </Label>
              <div className="col-span-3">
                <Input id="username" name="username" placeholder="(optional)" />
              </div>
            </div>

            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="password" className="text-right text-nowrap">
                Password
              </Label>
              <div className="col-span-3">
                <PasswordInput id="password" name="password" placeholder="(optional)" />
              </div>
            </div>

            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="authSource" className="text-right text-nowrap">
                Auth Source
                <TooltipProvider>
                  <Tooltip>
                    <TooltipTrigger>
                      <InfoIcon className="h-3 w-3 ml-1 inline" />
                    </TooltipTrigger>
                    <TooltipContent>
                      <p className="max-w-xs">The database used for authentication (usually 'admin' for MongoDB). <strong>Set to 'admin' to fix SCRAM authentication failures</strong>.</p>
                    </TooltipContent>
                  </Tooltip>
                </TooltipProvider>
              </Label>
              <div className="col-span-3">
                <Input id="authSource" name="authSource" defaultValue="admin" placeholder="admin (recommended)" />
              </div>
            </div>

            <Alert className="bg-blue-500/10 text-blue-600 border-blue-200 mt-2">
              <InfoIcon className="h-4 w-4" />
              <AlertDescription>
                To prevent "SCRAM authentication failure," Auth Source is set to "admin" by default
              </AlertDescription>
            </Alert>
          </div>
        </TabsContent>

        <TabsContent value="connectionString">
            <Textarea
              id="connectionString"
              name="connectionString"
              placeholder="Paste your MongoDB connection string here.
Database name is required at the end of the URL.

Examples:
- mongodb://localhost:27017/my_database
- mongodb://username:password@localhost:27017/my_database
- mongodb://username:password@localhost:27017/my_database?authSource=admin
- mongodb+srv://username:password@cluster.mongodb.net/my_database"
              required={connectionTab === "connectionString"}
              className="min-h-24"
            />
            <div className="text-xs text-muted-foreground mt-2">
              <p>For authenticated connections, make sure to include the username, password, and authSource if needed.</p>
              <p>Example with authentication: <code>mongodb://username:password@host:port/database?authSource=admin</code></p>
              <p className="mt-1 text-blue-500 font-medium">👉 If you encounter "SCRAM authentication failure," add <code>?authSource=admin</code> to your connection string.</p>
            </div>
        </TabsContent>
      </Tabs>

      {formError && (
        <Alert variant="destructive" className="mt-4">
          <AlertCircle className="h-4 w-4" />
          <AlertDescription>
            {formError}
          </AlertDescription>
        </Alert>
      )}

      {newConnMutation.isError && (
        <Alert variant="destructive" className="mt-4">
          <AlertCircle className="h-4 w-4" />
          <AlertDescription>
            {newConnMutation.error.message}
          </AlertDescription>
        </Alert>
      )}
    </form>
  );
} 
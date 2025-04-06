import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { type MongoDBConnectionMeta } from "@/stores/db-connections";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { useEditConnectionMutation } from "@/mutations/edit-connection";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { AlertCircle, InfoIcon } from "lucide-react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { PasswordInput } from "@/components/password-input";

interface EditMongoDBConnectionDialogProps {
  connection: MongoDBConnectionMeta;
  trigger: React.ReactNode;
}

export function EditMongoDBConnectionDialog({
  connection,
  trigger,
}: EditMongoDBConnectionDialogProps) {
  const [open, setOpen] = useState(false);
  const editMutation = useEditConnectionMutation();
  const [connectionTab, setConnectionTab] = useState<string>("connectionString");
  const [formError, setFormError] = useState<string | null>(null);

  // Parse connection string to get details (for form tab)
  const parseConnectionString = (connString: string) => {
    try {
      const url = new URL(connString.replace('mongodb://', 'http://').replace('mongodb+srv://', 'http://'));

      let host = url.hostname || 'localhost';
      let port = url.port || '27017';
      let database = url.pathname.replace('/', '') || '';
      let username = url.username || '';
      let password = url.password || '';

      // Extract auth source if available
      let authSource = '';
      if (url.search) {
        const params = new URLSearchParams(url.search);
        authSource = params.get('authSource') || '';
      }

      return { host, port, database, username, password, authSource };
    } catch (e) {
      // Fallback to basic parsing if URL parsing fails

      let parts = connString.split('@');
      let hostPart = parts.length > 1 ? parts[1] : parts[0];

      // Remove protocol prefix from hostPart if needed
      if (hostPart.startsWith('mongodb://')) {
        hostPart = hostPart.substring(10);
      } else if (hostPart.startsWith('mongodb+srv://')) {
        hostPart = hostPart.substring(14);
      }

      // Extract credentials
      let username = '';
      let password = '';
      if (parts.length > 1) {
        const credParts = parts[0]
          .replace('mongodb://', '')
          .replace('mongodb+srv://', '')
          .split(':');
        username = credParts[0] || '';
        password = credParts.length > 1 ? credParts[1] || '' : '';
      }

      // Extract host, port, database
      let [hostAndPort, ...dbParts] = hostPart.split('/');
      let database = dbParts.join('/') || '';

      // Extract auth source if available
      let authSource = '';
      if (database.includes('?')) {
        const dbAndParams = database.split('?');
        database = dbAndParams[0];
        const params = new URLSearchParams('?' + dbAndParams[1]);
        authSource = params.get('authSource') || '';
      }

      // Extract host and port
      let [host, port] = hostAndPort.split(':');
      host = host || 'localhost';
      port = port || '27017';

      return { host, port, database, username, password, authSource };
    }
  };

  const parsed = parseConnectionString(connection.connectionString);

  const handleSubmit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setFormError(null);

    const formData = new FormData(e.currentTarget);
    let connectionString: string;

    if (connectionTab === "connectionString") {
      connectionString = formData.get("connectionString") as string;

      if (!connectionString) return;

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
      connectionString = "mongodb://";

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
    }

    editMutation.mutate({
      connectionId: connection.id,
      dbType: 'mongodb',
      connectionString,
      tags: [],
    }, {
      onSuccess: () => {
        setOpen(false);
      }
    });
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      {trigger}
      <DialogContent className="sm:max-w-[550px]">
        <DialogHeader>
          <DialogTitle>Edit MongoDB Connection</DialogTitle>
          <DialogDescription>
            Update the connection details for this MongoDB database.
          </DialogDescription>
        </DialogHeader>

        <form id="edit-mongodb-form" onSubmit={handleSubmit}>
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
                    <Input id="host" name="host" defaultValue={parsed.host} placeholder="localhost" />
                  </div>
                </div>

                <div className="grid grid-cols-4 items-center gap-4">
                  <Label htmlFor="port" className="text-right text-nowrap">
                    Port
                  </Label>
                  <div className="col-span-3">
                    <Input id="port" name="port" defaultValue={parsed.port} placeholder="27017" />
                  </div>
                </div>

                <div className="grid grid-cols-4 items-center gap-4">
                  <Label htmlFor="database" className="text-right text-nowrap">
                    Database
                  </Label>
                  <div className="col-span-3">
                    <Input id="database" name="database" defaultValue={parsed.database} placeholder="my_database" required />
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
                    <Input id="username" name="username" defaultValue={parsed.username} placeholder="(optional)" />
                  </div>
                </div>

                <div className="grid grid-cols-4 items-center gap-4">
                  <Label htmlFor="password" className="text-right text-nowrap">
                    Password
                  </Label>
                  <div className="col-span-3">
                    <PasswordInput id="password" name="password" defaultValue={parsed.password} placeholder="(optional)" />
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
                    <Input id="authSource" name="authSource" defaultValue={parsed.authSource || "admin"} placeholder="admin (recommended)" />
                  </div>
                </div>

                <Alert className="bg-blue-500/10 text-blue-500 mt-2">
                  <InfoIcon className="size-4" />
                  <AlertDescription>
                    If you encounter "SCRAM authentication failure," set Auth Source to "admin"
                  </AlertDescription>
                </Alert>
              </div>
            </TabsContent>

            <TabsContent value="connectionString">
              <Textarea
                id="connectionString"
                name="connectionString"
                defaultValue={connection.connectionString}
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

          {editMutation.isError && (
            <Alert variant="destructive" className="mt-4">
              <AlertCircle className="h-4 w-4" />
              <AlertDescription>
                {editMutation.error.message}
              </AlertDescription>
            </Alert>
          )}
        </form>

        <DialogFooter>
          <Button
            type="submit"
            form="edit-mongodb-form"
            disabled={editMutation.isPending}
          >
            {editMutation.isPending ? "Saving..." : "Save Changes"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
} 
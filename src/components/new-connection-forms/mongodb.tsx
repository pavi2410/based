import { Input } from "@/components/ui/input.tsx";
import { Label } from "@/components/ui/label.tsx";
import { newConnectionMutation } from "@/mutations/new-connection";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { AlertCircle } from "lucide-react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useState } from "react";
import { Textarea } from "@/components/ui/textarea";

export function MongoDBConnectionForm() {
  const newConnMutation = newConnectionMutation();
  const [connectionTab, setConnectionTab] = useState<string>("form");

  const handleFormSubmit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    e.stopPropagation();

    const formData = new FormData(e.currentTarget);

    if (connectionTab === "connectionString") {
      // Handle direct connection string input
      const connectionString = formData.get("connectionString") as string;

      if (!connectionString) {
        return;
      }

      newConnMutation.mutate({
        dbType: "mongodb",
        filePath: connectionString,
      });
    } else {
      // Handle form-based connection
      const host = formData.get("host") as string || "localhost";
      const port = formData.get("port") as string || "27017";
      const database = formData.get("database") as string;
      const username = formData.get("username") as string;
      const password = formData.get("password") as string;
      
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
      
      // Add database name if provided
      if (database) {
        connectionString += `/${database}`;
      }
      
      newConnMutation.mutate({
        dbType: "mongodb",
        filePath: connectionString,
      });
    }
  };

  return (
    <form
      id="new-connection-form"
      onSubmit={handleFormSubmit}
    >
      <Tabs value={connectionTab} onValueChange={setConnectionTab} className="w-full">
        <TabsList className="grid w-full grid-cols-2 mb-4">
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
                <Input id="database" name="database" placeholder="my_database (optional)" />
              </div>
            </div>

            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="username" className="text-right text-nowrap">
                Username
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
                <Input id="password" name="password" type="password" placeholder="(optional)" />
              </div>
            </div>
          </div>
        </TabsContent>

        <TabsContent value="connectionString">
            <Textarea
              id="connectionString"
              name="connectionString"
              placeholder="Paste your MongoDB connection string here.

Examples:
- mongodb://localhost:27017
- mongodb://localhost:27017/mydb
- mongodb+srv://username:password@cluster.mongodb.net"
              required={connectionTab === "connectionString"}
              className="min-h-24"
            />
        </TabsContent>
      </Tabs>

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
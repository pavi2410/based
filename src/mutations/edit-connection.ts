import { toast } from "sonner";
import { updateConnection, getConnection } from "@/stores/db-connections";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { load } from "@/commands";
import { buildConnString } from "@/utils";

export function useEditConnectionMutation() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async ({
      connectionId,
      filePath,
    }: {
      connectionId: string;
      filePath: string;
    }) => {
      // Get existing connection to determine type
      const existingConnection = await getConnection(connectionId);
      if (!existingConnection) {
        throw new Error("Connection not found");
      }
      
      // Create connection string for validation
      const connectionString = buildConnString({
        ...existingConnection,
        filePath
      });
      
      // Validate the connection works
      try {
        await load(connectionString);
      } catch (error) {
        if (error instanceof Error) {
          // Handle specific error cases
          if (error.message.includes("Parent directory does not exist")) {
            throw new Error("The directory containing the database file does not exist");
          } else if (error.message.includes("No write permissions")) {
            throw new Error("You don't have permission to access this database file");
          } else if (error.message.includes("invalid connection url")) {
            throw new Error("Invalid database file path");
          } else if (error.message.includes("connection refused")) {
            throw new Error("Connection refused. Please check if the database server is running.");
          } else if (
            error.message.includes("authentication failed") || 
            error.message.includes("Authentication failed") ||
            error.message.includes("SCRAM failure")
          ) {
            throw new Error("Authentication failed. Please check your username and password.");
          } else if (error.message.includes("InvalidNamespace")) {
            throw new Error("Invalid database name. Database names cannot contain periods (.) or other special characters.");
          } else if (error.message.includes("No database name found")) {
            throw new Error("No database name specified. Please include a database name in your connection string.");
          }
        }
        throw error;
      }
      
      // Update the connection
      await updateConnection(connectionId, {
        ...existingConnection,
        filePath,
        updatedAt: Date.now()
      });
    },
    
    onSuccess: async () => {
      toast.success("Connection updated successfully");
      await queryClient.invalidateQueries({
        queryKey: ["connections"],
      });
    },
    
    onError: (error) => {
      toast.error(error.message);
    },
  });
} 
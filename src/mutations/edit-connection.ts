import { toast } from "sonner";
import { updateConnection, getConnection, EditableFields, ConnectionMeta, SqliteConnectionMeta, MongoDBConnectionMeta } from "@/stores/db-connections";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { buildConnString, testConnection } from "@/utils";
import { load } from "@/commands";

export function useEditConnectionMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (variables: EditableFields & { connectionId: string }) => {
      // Get existing connection to determine type
      const existingConnection = await getConnection(variables.connectionId);
      if (!existingConnection) {
        throw new Error("Connection not found");
      }

      // Create a new connection object with the updated values
      let newConnection: ConnectionMeta;
      if (existingConnection.dbType === "sqlite") {
        newConnection = {
          ...existingConnection,
          ...variables,
        } as SqliteConnectionMeta;
      } else {
        newConnection = {
          ...existingConnection,
          ...variables
        } as MongoDBConnectionMeta;
      }

      // Create connection string for validation
      const connectionString = buildConnString(newConnection);

      // Validate the connection works
      await load(connectionString);

      // Update the connection
      await updateConnection(variables.connectionId, newConnection);
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
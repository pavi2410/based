import { toast } from "sonner";
import { addConnection } from "@/stores";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { load } from "@/commands";

export function newConnectionMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      dbType, filePath,
    }: {
      dbType: string;
      filePath: string;
    }) => {
      console.log('Starting new connection with:', { dbType, filePath });
      
      // First try to load the connection to validate it
      const connString = `sqlite:${filePath}`;
      console.log('Attempting to load connection with string:', connString);
      
      try {
        await load(connString);
        console.log('Connection loaded successfully');
      } catch (error) {
        console.error('Error loading connection:', error);
        // Handle specific error cases
        if (error instanceof Error) {
          if (error.message.includes("Parent directory does not exist")) {
            console.error('Directory does not exist error');
            throw new Error("The directory containing the database file does not exist");
          } else if (error.message.includes("No write permissions")) {
            console.error('Permission error');
            throw new Error("You don't have permission to access this database file");
          } else if (error.message.includes("invalid connection url")) {
            console.error('Invalid URL error');
            throw new Error("Invalid database file path");
          }
        }
        console.error('Unhandled error:', error);
        throw error;
      }

      console.log('Adding connection to store');
      // If connection is successful, add it to the store
      await addConnection({
        dbType: dbType as "sqlite",
        filePath,
        groupName: "test",
      });
      console.log('Connection added to store successfully');
    },
    onSuccess: async () => {
      console.log('Mutation successful, showing toast');
      toast.success("New connection added");
      await queryClient.invalidateQueries({
        queryKey: ["connections"],
      });
    },
    onError: (error) => {
      console.error('Mutation error:', error);
      toast.error(error.message);
    },
  });
}
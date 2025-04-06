import { toast } from "sonner";
import { addConnection } from "@/stores/db-connections";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { load } from "@/commands";

export function newConnectionMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      dbType, filePath,
    }: {
      dbType: string;
      filePath: string | File | unknown;
    }) => {
      console.log('Starting new connection with:', { dbType, filePath });
      
      // Ensure filePath is a string
      let filePathStr: string;
      
      if (typeof filePath === 'string') {
        filePathStr = filePath;
      } else if (filePath instanceof File) {
        console.error('Received File object instead of path string:', filePath);
        throw new Error("Invalid file path: received File object instead of path string");
      } else {
        console.error('Invalid file path type:', typeof filePath, filePath);
        throw new Error("Invalid file path: must be a string");
      }
      
      // First try to load the connection to validate it
      let connString: string;
      
      if (dbType === 'sqlite') {
        connString = `sqlite:${filePathStr}`;
      } else if (dbType === 'mongodb') {
        connString = filePathStr; // MongoDB connection string is already in the correct format
      } else {
        throw new Error(`Unsupported database type: ${dbType}`);
      }
      
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
          } else if (error.message.includes("connection refused")) {
            console.error('Connection refused error');
            throw new Error("Connection refused. Please check if the MongoDB server is running.");
          } else if (
            error.message.includes("authentication failed") || 
            error.message.includes("Authentication failed") ||
            error.message.includes("SCRAM failure")
          ) {
            console.error('Authentication error');
            throw new Error("Authentication failed. Please check your username and password.");
          } else if (error.message.includes("InvalidNamespace")) {
            console.error('Invalid database name error');
            throw new Error("Invalid database name. Database names cannot contain periods (.) or other special characters.");
          } else if (error.message.includes("No database name found")) {
            console.error('Missing database name error');
            throw new Error("No database name specified. Please include a database name in your connection string.");
          }
        }
        console.error('Unhandled error:', error);
        throw error;
      }

      console.log('Adding connection to store');
      // If connection is successful, add it to the store
      await addConnection({
        dbType: dbType as "sqlite" | "mongodb",
        filePath: filePathStr,
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
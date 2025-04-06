import { load, query } from "@/commands";
import { useQuery } from "@tanstack/react-query";
import { DbConnectionMeta, getConnection } from "@/stores/db-connections";
import { buildConnString } from "@/utils";

// Define a status enum for the connection
export type ConnectionStatus = 
  | { status: 'loading' }
  | { status: 'error', error: Error }
  | { status: 'success', data: DbConnectionMeta };

export function useConnection(connId: string) {
  // First query to get connection metadata
  const connMetaQuery = useQuery({
    queryKey: ["connection", connId],
    queryFn: async () => {
      return await getConnection(connId);
    },
  });

  // Second dependent query to check connection status
  const connStatusQuery = useQuery({
    queryKey: ["connection-status", connId],
    queryFn: async () => {
      if (!connMetaQuery.data) {
        throw new Error("Connection not found");
      }
      
      const connMeta = connMetaQuery.data;
      const connString = buildConnString(connMeta);
      
      try {
        await load(connString);
        return { connMeta, isConnected: true };
      } catch (error) {
        if (error instanceof Error) {
          console.error('Error loading connection:', error);
          if (error.message.includes("Parent directory does not exist")) {
            throw new Error("The directory containing the database file does not exist");
          } else if (error.message.includes("No write permissions")) {
            throw new Error("You don't have permission to access this database file");
          } else if (error.message.includes("invalid connection url")) {
            throw new Error("Invalid database file path");
          }
        }
        throw error;
      }
    },
    enabled: connMetaQuery.isSuccess,
    retry: false,
  });

  // Create a status object that follows the ConnectionStatus type
  let status: ConnectionStatus;
  
  if (connMetaQuery.status === 'pending' || 
       (connMetaQuery.status === 'success' && connStatusQuery.status === 'pending')) {
    status = { status: 'loading' };
  } else if (connMetaQuery.status === 'error') {
    // Use type assertion to handle the unknown error type
    const error = connMetaQuery.error as unknown;
    
    // Handle the error based on its type
    let errorMessage: string;
    if (error instanceof Error) {
      errorMessage = error.message;
    } else if (typeof error === 'string') {
      errorMessage = error;
    } else if (error && typeof error === 'object' && 'message' in error) {
      errorMessage = String((error as { message: unknown }).message);
    } else {
      errorMessage = 'Unknown error loading connection metadata';
    }
    
    status = { 
      status: 'error',
      error: new Error(errorMessage)
    };
  } else if (connStatusQuery.status === 'error') {
    console.error({
      connStatusError: connStatusQuery.error,
      connMetaError: connMetaQuery.error,
    });
    
    // Handle the error properly regardless of its type
    let errorMessage: string;
    
    // Use type assertion to handle the unknown error type
    const error = connStatusQuery.error as unknown;
    
    // Handle the error based on its type
    if (error instanceof Error) {
      errorMessage = error.message;
    } else if (typeof error === 'string') {
      errorMessage = error;
    } else if (error && typeof error === 'object' && 'message' in error) {
      errorMessage = String((error as { message: unknown }).message);
    } else {
      errorMessage = 'Unknown error connecting to database';
    }
    
    status = { 
      status: 'error', 
      error: new Error(errorMessage)
    };
  } else if (connStatusQuery.status === 'success') {
    status = { 
      status: 'success', 
      data: connStatusQuery.data.connMeta 
    };
  } else {
    // Default to loading state for any other case
    status = { status: 'loading' };
  }

  // Create a retry function that handles both queries
  const retry = () => {
    console.log('Retrying connection...');
    
    // Use the status to determine what to retry
    if (status.status === 'error') {
      if (connMetaQuery.status === 'error') {
        console.log('Retrying metadata query...');
        connMetaQuery.refetch();
      } else if (connStatusQuery.status === 'error') {
        console.log('Retrying connection status query...');
        connStatusQuery.refetch();
      } else {
        // Fallback: retry both
        console.log('Retrying both queries...');
        connMetaQuery.refetch();
        if (connMetaQuery.status === 'success') {
          connStatusQuery.refetch();
        }
      }
    } else {
      // No errors, but user wants to retry anyway
      console.log('No errors, but retrying anyway...');
      connMetaQuery.refetch();
      if (connMetaQuery.status === 'success') {
        connStatusQuery.refetch();
      }
    }
  };

  return {
    status,
    retry,
  };
} 
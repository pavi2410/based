import { toast } from "sonner";
import { addConnection, EditableFields } from "@/stores/db-connections";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { buildConnString, testConnection } from "@/utils";

export function newConnectionMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (variables: EditableFields) => {
      let connString = buildConnString(variables);
      
      // First try to load the connection to validate it
      await testConnection(connString);

      // If the connection is valid, add it to the store
      await addConnection(variables);
    },
    onSuccess: async () => {
      toast.success("New connection added");
      await queryClient.invalidateQueries({
        queryKey: ["connections"],
      });
    },
    onError: (error) => {
      toast.error(error.message);
    },
  });
}
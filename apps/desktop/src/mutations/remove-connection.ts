import { removeConnection } from "@/stores/db-connections";
import { useMutation, useQueryClient } from "@tanstack/react-query";

export function useRemoveConnectionMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: removeConnection,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["connections"] });
    },
  });
}
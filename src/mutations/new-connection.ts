import { toast } from "@/hooks/use-toast";
import { addConnection } from "@/stores";
import { useMutation, useQueryClient } from "@tanstack/react-query";

export function newConnectionMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      dbType, filePath,
    }: {
      dbType: string;
      filePath: string;
    }) => {
      await addConnection({
        dbType: dbType as "sqlite",
        filePath,
        groupName: "test",
      });
    },
    onSuccess: async () => {
      toast({
        title: "New connected added",
      });
      await queryClient.invalidateQueries({
        queryKey: ["connections"],
      });
    },
  });
}
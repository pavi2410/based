import { getConnections } from "@/stores/db-connections";
import { useQuery } from "@tanstack/react-query";

export function useConnectionList() {
  return useQuery({
    queryKey: ["connections"],
    queryFn: getConnections,
  });
}
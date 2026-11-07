import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap } from "../../ipc";

export function useCollections() {
  return useQuery({ queryKey: ["collections"], queryFn: () => unwrap(commands.listCollections()) });
}

/** Creates a collection (smart when `query` is given) and calls `onCreated` with it. */
export function useCreateCollection(onError: (message: string) => void, onCreated?: (id: number) => void) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ name, query }: { name: string; query: string | null }) => unwrap(commands.createCollection(name, query)),
    onSuccess: (collection) => {
      void queryClient.invalidateQueries({ queryKey: ["collections"] });
      onCreated?.(collection.id);
    },
    onError: (error) => onError(describeError(error)),
  });
}

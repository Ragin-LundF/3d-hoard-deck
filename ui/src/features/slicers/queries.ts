import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, unwrap, type SlicerDto } from "../../ipc";

export function useSlicers() {
  return useQuery({ queryKey: ["slicers"], queryFn: () => unwrap(commands.listSlicers()) });
}

/** Saves the whole list (first = default) and refreshes everyone showing it. */
export function useSaveSlicers(onError: (error: unknown) => void) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (slicers: SlicerDto[]) => unwrap(commands.saveSlicers(slicers)),
    onSuccess: (list) => queryClient.setQueryData(["slicers"], list),
    onError,
  });
}

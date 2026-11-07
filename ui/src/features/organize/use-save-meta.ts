import { useMutation, useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap, type ModelDetailDto_Serialize, type ModelMetaDto } from "../../ipc";

/** Saves a model's name, tags, marks, notes and cover; the model page updates at once, grids and tags on success. */
export function useMetaSaver(onError: (message: string) => void) {
  const queryClient = useQueryClient();
  const mutation = useMutation({
    mutationFn: ({ id, meta }: { id: number; meta: ModelMetaDto }) => unwrap(commands.updateModelMeta(id, meta)),
    onSuccess: (detail) => {
      queryClient.setQueryData(["model", detail.summary.id], detail);
      void queryClient.invalidateQueries({ predicate: (query) => query.queryKey[0] !== "model" });
    },
    onError: (error) => onError(describeError(error)),
  });
  return (model: ModelDetailDto_Serialize, change: Partial<ModelMetaDto>) =>
    mutation.mutate({ id: model.summary.id, meta: { ...model.meta, ...change } });
}

/** {@link useMetaSaver} bound to one model. */
export function useSaveMeta(model: ModelDetailDto_Serialize, onError: (message: string) => void) {
  const save = useMetaSaver(onError);
  return (change: Partial<ModelMetaDto>) => save(model, change);
}

import { useEffect, useSyncExternalStore } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, events, unwrap, type SuggestProgressEvent } from "../../ipc";

/** AI settings; `enabled` is false until the user picks a provider, and then no AI control renders anywhere. */
export function useAiSettings() {
  return useQuery({ queryKey: ["ai"], queryFn: () => unwrap(commands.aiSettings()) });
}

export function useAiEnabled(): boolean {
  const provider = useAiSettings().data?.settings.provider;
  return provider !== undefined && provider !== "off";
}

let progress: SuggestProgressEvent | null = null;
const listeners = new Set<() => void>();
const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

/** The background run's progress, shared by the status bar and the buttons that start runs. */
export function useSuggestProgress(): SuggestProgressEvent | null {
  return useSyncExternalStore(subscribe, () => progress);
}

/** Listens once, in the app shell; refreshes suggestions while a run goes on. */
export function useSuggestProgressListener() {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = events.suggestProgressEvent.listen(({ payload }) => {
      progress = payload;
      listeners.forEach((listener) => listener());
      void queryClient.invalidateQueries({ queryKey: ["suggestions"] });
      if (!payload.running) void queryClient.invalidateQueries({ queryKey: ["suggestedTags"] });
    });
    return () => void unlisten.then((stop) => stop());
  }, [queryClient]);
}

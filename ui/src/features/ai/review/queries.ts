import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap, type DecisionDto } from "../../../ipc";
import { logReview, markUndone, tally, type LogEntry } from "./review-log";

export function useReviewItems(library: number, folder: string) {
  return useQuery({ queryKey: ["review", library, folder], queryFn: () => unwrap(commands.reviewSuggestions(library, folder, null)) });
}

/** Applies decisions, logs them for undo, and refreshes everything that shows tags. */
export function useDecide(library: number, onMessage: (message: string) => void) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ decisions, label }: { decisions: DecisionDto[]; label: string }) => {
      const resolutions = await unwrap(commands.decideSuggestions(library, decisions));
      return logReview({ library, label, resolutions, ...tally(resolutions) });
    },
    onSuccess: () => void queryClient.invalidateQueries(),
    onError: (error) => onMessage(describeError(error)),
  });
}

export function useUndo(onMessage: (message: string) => void) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (entry: LogEntry) => {
      await unwrap(commands.undoSuggestions(entry.library, entry.resolutions));
      markUndone(entry.id);
    },
    onSuccess: () => void queryClient.invalidateQueries(),
    onError: (error) => onMessage(describeError(error)),
  });
}

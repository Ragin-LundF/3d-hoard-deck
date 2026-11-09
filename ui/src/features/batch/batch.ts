import { useSyncExternalStore } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap } from "../../ipc";
import { plural, t } from "../../shared/i18n";

/** What a batch targets: picked models, or every result of a search in a folder. */
export type BatchTarget = { ids: number[] } | { query: string; folder: string };

type Undoable = { label: string; undo: () => Promise<unknown> };
let last: Undoable | null = null;
const listeners = new Set<() => void>();
const show = (next: Undoable | null) => {
  last = next;
  listeners.forEach((listener) => listener());
};

/** The last batch, shown with Undo until closed or replaced. */
export function useLastBatch(): Undoable | null {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => last,
  );
}

/** What a tag change can change on screen. */
const AFFECTED = ["tags", "tag-facets", "model", "search", "check", "review", "browse", "folder"];

/** Adds and removes tags in one go; afterwards an Undo toast. */
export function useBatchTags(library: number, onMessage: (message: string) => void) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ target, add, remove }: { target: BatchTarget; add: string[]; remove: string[] }) => {
      const ids = "ids" in target ? target.ids : null;
      const query = "query" in target ? target.query : null;
      const folder = "folder" in target ? target.folder : "";
      const result = await unwrap(commands.batchUpdateTags(library, ids, query, folder, add, remove));
      const what = [...add.map((tag) => `+${tag}`), ...remove.map((tag) => `−${tag}`)].join(" ");
      const token = result.token;
      show({ undo: () => unwrap(commands.undoTagBatch(token)), label: plural(result.changed, "{what} on {count} model", "{what} on {count} models").replace("{what}", what) });
      return result;
    },
    onSuccess: () => void queryClient.invalidateQueries({ predicate: (query) => AFFECTED.includes(String(query.queryKey[0])) }),
    onError: (error) => onMessage(describeError(error)),
  });
}

/** The Undo toast of the last batch, mounted once in the app shell. */
export function useUndoBatch(onMessage: (message: string) => void) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (undoable: Undoable) => undoable.undo(),
    onSuccess: () => {
      show(null);
      void queryClient.invalidateQueries({ predicate: (query) => AFFECTED.includes(String(query.queryKey[0])) });
      onMessage(t("Undone."));
    },
    onError: (error) => onMessage(describeError(error)),
  });
}

export const closeLastBatch = () => show(null);

/** Shows any undoable change in the same toast (rendered previews use it too). */
export const showUndo = (label: string, undo: () => Promise<unknown>) => show({ label, undo });

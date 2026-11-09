import { useSyncExternalStore } from "react";
import type { LibraryDto } from "../../ipc";
import type { ArchiveItem, PendingExtraction } from "./extract-dialog";

/** An extraction the user started; shown by the app shell so it survives leaving the page it started on. */
export type ExtractRequest = { library: LibraryDto; items: ArchiveItem[]; pending?: PendingExtraction[] };

let current: ExtractRequest | null = null;
const listeners = new Set<() => void>();
const set = (next: ExtractRequest | null) => {
  current = next;
  listeners.forEach((listener) => listener());
};

/** Opens the extraction dialog, unless one is already running. */
export const openExtract = (request: ExtractRequest) => {
  if (!current) set(request);
};
export const closeExtract = () => set(null);

export function useExtractRequest(): ExtractRequest | null {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => current,
  );
}

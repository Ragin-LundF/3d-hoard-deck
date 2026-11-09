import { useSyncExternalStore } from "react";
import type { ResolutionDto } from "../../../ipc";

/** One review action of this session, kept so it can be undone from the "Reviewed" tab. */
export type LogEntry = { id: number; library: number; label: string; kept: number; dropped: number; resolutions: ResolutionDto[]; undone: boolean };

let entries: LogEntry[] = [];
let next = 1;
const listeners = new Set<() => void>();
const emit = () => listeners.forEach((listener) => listener());

export function logReview(entry: Omit<LogEntry, "id" | "undone">): LogEntry {
  const logged = { ...entry, id: next++, undone: false };
  entries = [logged, ...entries].slice(0, 200);
  emit();
  return logged;
}

export function markUndone(id: number) {
  entries = entries.map((entry) => (entry.id === id ? { ...entry, undone: true } : entry));
  emit();
}

export function useReviewLog(library: number): LogEntry[] {
  const all = useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => entries,
  );
  return all.filter((entry) => entry.library === library);
}

/** How many tags a set of resolutions kept and dropped. */
export function tally(resolutions: ResolutionDto[]): { kept: number; dropped: number } {
  const kept = resolutions.reduce((sum, r) => sum + r.added.length, 0);
  const removed = resolutions.reduce((sum, r) => sum + r.removed.length, 0);
  return { kept, dropped: removed - kept };
}

import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { events, IpcFailure, type IpcError } from "../../ipc";

/** `unreadable`: folders and files the scan could not read (their old data is kept), with the first as examples. */
export type ScanStatus = {
  phase: string;
  folders: number;
  files: number;
  unreadable: number;
  unreadableExamples: string[];
  error: IpcError | null;
};

const STORAGE_KEY = "lastScanProblems";

/** What the last finished scan could not read, so the list survives a restart or a language switch. */
function remembered(): Record<number, ScanStatus> {
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") as Record<number, { unreadable: number; unreadableExamples: string[] }>;
    return Object.fromEntries(
      Object.entries(stored).map(([library, problems]) => [library, { phase: "done", folders: 0, files: 0, error: null, ...problems }]),
    );
  } catch {
    return {};
  }
}

function remember(library: number, unreadable: number, unreadableExamples: string[]) {
  const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") as Record<number, unknown>;
  if (unreadable > 0) stored[library] = { unreadable, unreadableExamples };
  else delete stored[library];
  localStorage.setItem(STORAGE_KEY, JSON.stringify(stored));
}

/** Latest scan progress per library; refreshes every query when a scan ends and reports each failure once. */
export function useScanStatus(onFailed: (error: IpcFailure) => void): Record<number, ScanStatus> {
  const [status, setStatus] = useState<Record<number, ScanStatus>>(remembered);
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = events.scanProgressEvent.listen(({ payload }) => {
      setStatus((current) => ({
        ...current,
        [payload.library]: {
          phase: payload.phase,
          folders: payload.folders ?? 0,
          files: payload.files ?? 0,
          // A folder scan only knows its own part; the last full scan's list stays.
          unreadable: payload.partial ? (current[payload.library]?.unreadable ?? 0) : (payload.unreadable ?? 0),
          unreadableExamples: payload.partial ? (current[payload.library]?.unreadableExamples ?? []) : payload.unreadableExamples,
          error: payload.error ?? null,
        },
      }));
      if (payload.phase === "done" && !payload.partial) remember(payload.library, payload.unreadable ?? 0, payload.unreadableExamples);
      if (payload.phase === "done" || payload.phase === "failed") void queryClient.invalidateQueries();
      if (payload.error) onFailed(new IpcFailure(payload.error));
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [queryClient, onFailed]);
  return status;
}

export function isScanning(status: ScanStatus | undefined): boolean {
  return status !== undefined && status.phase !== "done" && status.phase !== "failed";
}

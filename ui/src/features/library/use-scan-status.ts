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

/** Latest scan progress per library; refreshes every query when a scan ends and reports each failure once. */
export function useScanStatus(onFailed: (error: IpcFailure) => void): Record<number, ScanStatus> {
  const [status, setStatus] = useState<Record<number, ScanStatus>>({});
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = events.scanProgressEvent.listen(({ payload }) => {
      setStatus((current) => ({
        ...current,
        [payload.library]: {
          phase: payload.phase,
          folders: payload.folders ?? 0,
          files: payload.files ?? 0,
          unreadable: payload.unreadable ?? 0,
          unreadableExamples: payload.unreadableExamples,
          error: payload.error ?? null,
        },
      }));
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

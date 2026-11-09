import { useEffect, useSyncExternalStore } from "react";
import { commands, events, IpcFailure, unwrap, type UpdateInfoDto_Serialize } from "../../ipc";
import { checksOnStart, hiddenBySkip, skippedVersion } from "./update-prefs";

/** A few seconds after start, so the first screen and the first scan come first. */
const START_DELAY = 4000;

export type UpdateState = {
  info: UpdateInfoDto_Serialize | null;
  /** "Later" or "Skip": hidden until the next start (or a newer version). */
  hidden: boolean;
  phase: "idle" | "downloading" | "restarting" | "error";
  done: number;
  total: number;
  error: string | null;
};

let state: UpdateState = { info: null, hidden: false, phase: "idle", done: 0, total: 0, error: null };
const listeners = new Set<() => void>();
const set = (patch: Partial<UpdateState>) => {
  state = { ...state, ...patch };
  listeners.forEach((listener) => listener());
};
const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

export const useUpdate = () => useSyncExternalStore(subscribe, () => state);
export const hideUpdate = () => set({ hidden: true });

/** Asks GitHub. Returns the newer version (also when skipped, for "Check now"), or null. */
export async function checkForUpdate(): Promise<UpdateInfoDto_Serialize | null> {
  const info = await unwrap(commands.checkForUpdate());
  set({ info, hidden: info !== null && hiddenBySkip(info.version, skippedVersion()) });
  return info;
}

export async function installUpdate() {
  set({ phase: "downloading", done: 0, total: 0, error: null });
  const result = await commands.installUpdate();
  // Success restarts the app, so only a failure comes back here.
  if (result.status === "error") set({ phase: "error", error: new IpcFailure(result.error).message });
}

/** Mounted once in the app shell: listens for download progress and checks once after start. */
export function useUpdateCheck() {
  useEffect(() => {
    const unlisten = events.updateProgressEvent.listen(({ payload }) =>
      set({ done: payload.done, total: payload.total, phase: payload.total > 0 && payload.done >= payload.total ? "restarting" : "downloading" }),
    );
    const timer = checksOnStart()
      ? setTimeout(() => checkForUpdate().catch((error) => console.warn("update check failed", error)), START_DELAY)
      : undefined;
    return () => {
      clearTimeout(timer);
      void unlisten.then((stop) => stop());
    };
  }, []);
}

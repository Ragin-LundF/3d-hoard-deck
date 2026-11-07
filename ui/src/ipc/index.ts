import { convertFileSrc } from "@tauri-apps/api/core";
import type { IpcError } from "./bindings";

export * from "./bindings";

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: IpcError };

/** Turns a command result into a value or a thrown [`IpcFailure`], which TanStack Query expects. */
export async function unwrap<T>(result: Promise<Result<T>>): Promise<T> {
  const settled = await result;
  if (settled.status === "error") throw new IpcFailure(settled.error);
  return settled.data;
}

export class IpcFailure extends Error {
  readonly code: string;
  readonly details: string | null;

  constructor(error: IpcError) {
    super(error.message);
    this.code = error.code;
    this.details = error.details ?? null;
  }
}

const MESSAGES: Record<string, string> = {
  offline: "The library folder is not reachable. Is the drive or NAS connected?",
  library_busy: "This library is being scanned right now. Try again when the scan is done.",
  invalid_input: "That did not work: the input was not valid.",
  library_not_found: "This library no longer exists.",
  no_slicer: "None of your slicers opens these files. Add one or adjust its file types in Settings › Slicers.",
  launch_failed: "The slicer could not be started. Check its program path in Settings › Slicers.",
  unsafe_location: "Network shares and cloud-sync folders (iCloud, Dropbox, OneDrive, Google Drive) damage the index database. Choose a folder on a local disk.",
};

/** A message for the user, chosen by error code, never by matching message text. */
export function describeError(error: unknown): string {
  if (error instanceof IpcFailure) return MESSAGES[error.code] ?? error.message;
  return error instanceof Error ? error.message : String(error);
}

/** URL of a library file, served read-only by the shell's `hoard://` protocol. */
export function fileUrl(library: number, path: string): string {
  return convertFileSrc(`${library}/${path}`, "hoard");
}

/** URL of a small cached thumbnail of a library image, for grids and galleries. */
export function thumbUrl(library: number, path: string): string {
  return convertFileSrc(`thumb/${library}/${path}`, "hoard");
}

/** URL of a 3MF's build plates, read and flattened by the shell. */
export function platesUrl(library: number, path: string): string {
  return convertFileSrc(`plates/${library}/${path}`, "hoard");
}

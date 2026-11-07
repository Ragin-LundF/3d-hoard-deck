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

  constructor(error: IpcError) {
    super(error.message);
    this.code = error.code;
  }
}

const MESSAGES: Record<string, string> = {
  offline: "The library folder is not reachable. Is the drive or NAS connected?",
  library_busy: "This library is being scanned right now. Try again when the scan is done.",
  invalid_input: "That did not work: the input was not valid.",
  library_not_found: "This library no longer exists.",
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

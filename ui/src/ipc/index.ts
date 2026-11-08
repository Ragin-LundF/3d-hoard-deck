import { convertFileSrc } from "@tauri-apps/api/core";
import type { IpcError } from "./bindings";
import { t } from "../shared/i18n";

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
  offline: t("The library folder is not reachable. Is the drive or NAS connected?"),
  library_busy: t("This library is being scanned right now. Try again when the scan is done."),
  library_not_found: t("This library no longer exists."),
  no_slicer: t("None of your slicers opens these files. Add one or adjust its file types in Settings › Slicers."),
  launch_failed: t("The slicer could not be started. Check its program path in Settings › Slicers."),
  unsafe_location: t("Network shares and cloud-sync folders (iCloud, Dropbox, OneDrive, Google Drive) damage the index database. Choose a folder on a local disk."),
};

/** Refusals name what they concern in `details` (a path, name or library). */
function refusal(code: string, subject: string): string | undefined {
  switch (code) {
    case "already_exists": return t("{path} already exists. Nothing was overwritten.", { path: subject });
    case "empty_name": return t("A name cannot be empty.");
    case "forbidden_character": return t("“{name}” contains a character Windows and network shares cannot store: \\ / : * ? \" < > |", { name: subject });
    case "trailing_dot_or_space": return t("“{name}” ends with a dot or a space, which Windows and network shares cannot store.", { name: subject });
    case "reserved_name": return t("“{name}” is reserved by Windows for devices. Choose another name.", { name: subject });
    case "smart_collection": return t("A smart collection fills itself from its search; models cannot be added by hand.");
    case "read_only": return t("{library} does not allow writing files. Change it under Settings › Libraries.", { library: subject });
    case "not_managed": return t("{library} does not allow renaming or moving files. Change it under Settings › Libraries.", { library: subject });
    default: return undefined;
  }
}

/** A message for the user, chosen by error code, never by matching message text. */
export function describeError(error: unknown): string {
  // Other `invalid_input` messages come from the backend in English and name the reason, so they are shown as written.
  if (error instanceof IpcFailure) {
    return refusal(error.code, error.details ?? "") ?? MESSAGES[error.code] ?? error.message.replace(/^invalid input: /, "");
  }
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

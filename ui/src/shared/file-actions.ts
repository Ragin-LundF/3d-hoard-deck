import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { t } from "./i18n";

/** Absolute path on this machine, in the root's own separator style. */
export function absolutePath(root: string, path: string): string {
  const separator = root.includes("\\") ? "\\" : "/";
  if (!path) return root;
  return `${root.replace(/[\\/]+$/, "")}${separator}${path.split("/").join(separator)}`;
}

/** Shows the file or folder in Finder / Explorer. */
export function revealPath(root: string, path: string, onMessage: (message: string) => void) {
  revealItemInDir(absolutePath(root, path)).catch((error) => onMessage(String(error)));
}

/** Puts the absolute path on the clipboard. */
export function copyPath(root: string, path: string, onMessage: (message: string) => void) {
  navigator.clipboard
    .writeText(absolutePath(root, path))
    .then(() => onMessage(t("Path copied.")))
    .catch((error) => onMessage(String(error)));
}

import type { AiFoldersDto } from "../../../ipc";

const within = (path: string, folder: string) => folder === "" || path === folder || path.startsWith(`${folder}/`);
const deepest = (path: string, list: string[]) =>
  Math.max(-1, ...list.filter((folder) => within(path, folder)).map((folder) => (folder === "" ? 0 : folder.split("/").length)));

/** Whether the AI works on `path`: the deepest ticked or unticked folder above it decides (as in the backend). */
export function wants(folders: AiFoldersDto, path: string): boolean {
  const included = deepest(path, folders.included);
  return included >= 0 && included > deepest(path, folders.excluded);
}

/** Ticked, unticked, or mixed (some folder below decides otherwise). */
export function stateOf(folders: AiFoldersDto, path: string): "on" | "off" | "mixed" {
  const on = wants(folders, path);
  const below = (list: string[]) => list.some((folder) => folder !== path && within(folder, path));
  return (on ? below(folders.excluded) : below(folders.included)) ? "mixed" : on ? "on" : "off";
}

/** Ticks or unticks `path` with everything below it, keeping the lists as short as possible. */
export function toggle(folders: AiFoldersDto, path: string): AiFoldersDto {
  const turnOn = stateOf(folders, path) !== "on";
  const outside = (list: string[]) => list.filter((folder) => !within(folder, path));
  const next = { ...folders, included: outside(folders.included), excluded: outside(folders.excluded) };
  if (wants(next, path) !== turnOn) {
    if (turnOn) next.included = [...next.included, path];
    else next.excluded = [...next.excluded, path];
  }
  return next;
}

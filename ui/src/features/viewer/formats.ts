// Kept apart from the loaders so the library screens do not pull in three.js.
export type MeshFormat = "stl" | "3mf" | "obj";

/** Which viewer format a file kind maps to; `null` for files the viewer cannot show. */
export function viewerFormat(kind: string): MeshFormat | null {
  return kind === "stl" || kind === "3mf" || kind === "obj" ? kind : null;
}

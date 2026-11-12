import { describe, expect, it, vi } from "vitest";

// The menus' modules load the translations, which read the browser's storage and set the page language: English here.
vi.hoisted(() => {
  Object.assign(globalThis, {
    localStorage: { getItem: () => "en", setItem: () => {}, removeItem: () => {} },
    document: { documentElement: {} },
  });
});
import type { LibraryDto, SlicerDto } from "../ipc";
import { folderMenuItems } from "../features/folders/folder-menu";
import { modelMenuItems, type ModelMenuEnv } from "../features/library/model-menu";
import { tagMenuItems } from "../features/tags/tag-menu";
import { tidy, type MenuItem } from "./context-menu";

/** The labels a menu shows, `✓` for a checked item, `-` for a line. */
const shown = (items: MenuItem[]) =>
  tidy(items).map((item) => (item === "separator" ? "-" : "label" in item ? `${item.label}${item.checked ? " ✓" : ""}` : "(hint)"));

const nothing = () => {};
const library = { id: 1, name: "3D Print", root: "/Volumes/nas/3D Print", access: "managed", metadata: "indexOnly" } as unknown as LibraryDto;
const bambu: SlicerDto = { name: "Bambu Studio", program: "", args: [], extensions: ["stl", "3mf"] };
const lychee: SlicerDto = { name: "Lychee", program: "", args: [], extensions: ["lys"] };
const env = (changes: Partial<ModelMenuEnv> = {}): ModelMenuEnv => ({
  library, slicer: bambu, open: nothing, openInSlicer: nothing, reveal: nothing, copy: nothing, mark: nothing, addToCollection: nothing, removeFromCollection: nothing, ...changes,
});
const model = { id: 7, library: 1, path: "Minis/Dragon", favorite: true, printed: false, formats: ["stl"] };

describe("model menu", () => {
  it("offers the default slicer only when it opens the model's files", () => {
    expect(shown(modelMenuItems(model, {}, env()))).toContain("Open in Bambu Studio");
    expect(shown(modelMenuItems(model, {}, env({ slicer: undefined })))).not.toContain("Open in Bambu Studio");
    expect(shown(modelMenuItems(model, {}, env({ slicer: lychee })))).not.toContain("Open in Lychee");
    expect(shown(modelMenuItems({ ...model, formats: [] }, {}, env())).join()).not.toContain("Open in");
  });

  it("shows marks with their state, and select and remove only where they apply", () => {
    expect(shown(modelMenuItems(model, {}, env()))).toEqual([
      "Open", "Open in Bambu Studio", "Show in folder", "-", "Favorite ✓", "Printed", "Add to collection…", "-", "Copy path",
    ]);
    const inCollection = shown(modelMenuItems(model, { collection: 3, onSelect: nothing }, env()));
    expect(inCollection).toContain("Select");
    expect(inCollection[inCollection.length - 1]).toBe("Remove from collection");
    const unknown = shown(modelMenuItems({ id: 7, library: 1, path: "a.stl" }, {}, env({ library: undefined })));
    expect(unknown).toEqual(["Open", "Open in Bambu Studio", "-", "Add to collection…"]);
  });
});

describe("folder menu", () => {
  it("checks the collection mark only when the role is known", () => {
    const actions = { open: nothing, reveal: nothing, copy: nothing, setRole: nothing, automaticCover: nothing, scan: nothing };
    expect(shown(folderMenuItems(true, actions))).toContain("Treat as collection ✓");
    expect(shown(folderMenuItems(undefined, actions))).toContain("Treat as collection");
  });
});

describe("tag menu", () => {
  it("toggles pin wording and offers check and remove only in the tag bar", () => {
    const actions = { show: nothing, exclude: nothing, pin: nothing, rename: nothing };
    expect(shown(tagMenuItems(false, actions))).toEqual(["Show models with this tag", "Exclude this tag", "Pin to the sidebar", "Rename…"]);
    const bar = shown(tagMenuItems(true, { ...actions, check: nothing, removeAll: nothing }));
    expect(bar).toContain("Unpin from the sidebar");
    expect(bar.slice(-3)).toEqual(["Check this tag…", "-", "Remove from all here…"]);
  });
});

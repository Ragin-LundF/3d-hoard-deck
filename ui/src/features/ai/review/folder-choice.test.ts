import { describe, expect, it } from "vitest";
import { stateOf, toggle, wants } from "./folder-choice";

const none = { included: [], excluded: [], onlyUntagged: false, afterScan: false, skipWithoutPreview: false };

describe("AI folder choice", () => {
  it("ticks a folder with everything below and unticks a subfolder inside it", () => {
    let folders = toggle(none, "Miniatures");
    expect(wants(folders, "Miniatures/Loot/Orc")).toBe(true);
    folders = toggle(folders, "Miniatures/Loot");
    expect(folders).toMatchObject({ included: ["Miniatures"], excluded: ["Miniatures/Loot"] });
    expect(stateOf(folders, "Miniatures")).toBe("mixed");
    expect(wants(folders, "Miniatures/Loot/Orc")).toBe(false);
  });

  it("ticking a mixed folder again clears the exceptions below it", () => {
    const folders = toggle(toggle(toggle(none, "Miniatures"), "Miniatures/Loot"), "Miniatures");
    expect(folders).toMatchObject({ included: ["Miniatures"], excluded: [] });
    expect(toggle(folders, "Miniatures")).toMatchObject({ included: [], excluded: [] });
  });
});

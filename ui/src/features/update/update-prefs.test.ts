import { describe, expect, it } from "vitest";
import { hiddenBySkip, newer, whatsNew } from "./update-prefs";

describe("update preferences", () => {
  it("compares versions by number", () => {
    expect(newer("1.10.0", "1.9.2")).toBe(true);
    expect(newer("1.0.0", "1.0.0")).toBe(false);
    expect(newer("v1.1", "1.0.9")).toBe(true);
  });

  it("keeps a skipped version hidden until a newer one comes", () => {
    expect(hiddenBySkip("1.1.0", "1.1.0")).toBe(true);
    expect(hiddenBySkip("1.2.0", "1.1.0")).toBe(false);
    expect(hiddenBySkip("1.1.0", null)).toBe(false);
  });

  it("takes the first sentence of the first list items as what's new", () => {
    const notes = "### New\n- **Tag rules.** Tags from folder names. More text.\n- AI suggestions.\n- Third.\n- Fourth.\n\n---\nInstallers…";
    expect(whatsNew(notes)).toEqual(["Tag rules.", "AI suggestions.", "Third."]);
  });
});

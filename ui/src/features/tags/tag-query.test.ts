import { describe, expect, it } from "vitest";
import { readTags, setMode, toggleExclude, toggleInclude } from "./tag-query";

describe("tag filters in the search text", () => {
  it("adds and removes required tags and keeps the rest of the search", () => {
    const one = toggleInclude("dragon", "Resin");
    expect(one).toBe("dragon tag:Resin");
    expect(toggleInclude(one, "Sci Fi")).toBe('dragon tag:Resin tag:"Sci Fi"');
    expect(toggleInclude(one, "resin")).toBe("dragon");
  });

  it("excludes with ⌥ and switches between all and any", () => {
    const excluded = toggleExclude("tag:resin", "bust");
    expect(excluded).toBe("tag:resin -tag:bust");
    // Clicking an excluded chip drops it.
    expect(toggleInclude(excluded, "bust")).toBe("tag:resin");
    const any = setMode("tag:32mm tag:28mm", "any");
    expect(any).toBe("tag:32mm|28mm");
    expect(readTags(any).any).toEqual(["32mm", "28mm"]);
    expect(toggleInclude(any, "75mm")).toBe("tag:32mm|28mm|75mm");
    // One tag left is simply required.
    expect(toggleInclude(any, "32mm")).toBe("tag:28mm");
    expect(setMode(any, "all")).toBe("tag:32mm tag:28mm");
  });

  it("reads quoted tags with spaces", () => {
    expect(readTags('-tag:"Sci Fi" tag:"28 mm|32 mm" knight')).toEqual({
      include: [],
      exclude: ["Sci Fi"],
      any: ["28 mm", "32 mm"],
      rest: ["knight"],
    });
  });
});

import { describe, expect, it } from "vitest";
import { likelyDuplicates } from "./duplicates";

describe("likely duplicate tags", () => {
  it("finds case, punctuation and plural variants and puts the most used first", () => {
    const tags = [
      { name: "Sci-Fi", count: 3 },
      { name: "SciFi", count: 9 },
      { name: "Dragons", count: 1 },
      { name: "dragon", count: 4 },
      { name: "Bus", count: 2 },
      { name: "Resin", count: 5 },
    ];
    expect(likelyDuplicates(tags).map((group) => group.map((tag) => tag.name))).toEqual([
      ["SciFi", "Sci-Fi"],
      ["dragon", "Dragons"],
    ]);
  });
});

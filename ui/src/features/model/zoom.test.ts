import { describe, expect, it } from "vitest";
import { clamp, fitted } from "./zoom";

const box = { width: 800, height: 600 };

describe("image zoom", () => {
  it("enlarges a small image to the box and keeps its shape", () => {
    expect(fitted(box, { width: 200, height: 100 })).toEqual({ width: 800, height: 400 });
  });

  it("never zooms out below the box size or past the limit", () => {
    expect(clamp({ zoom: 0.3, x: 0, y: 0 }, box, box).zoom).toBe(1);
    expect(clamp({ zoom: 50, x: 0, y: 0 }, box, box).zoom).toBe(8);
  });

  it("lets a zoomed image move only as far as its edges", () => {
    // 2× zoom of a box-sized image: 400 px to spare on each side horizontally, 300 vertically.
    expect(clamp({ zoom: 2, x: 1000, y: -1000 }, box, box)).toEqual({ zoom: 2, x: 400, y: -300 });
    // At box size nothing can move.
    expect(clamp({ zoom: 1, x: 50, y: 50 }, box, box)).toEqual({ zoom: 1, x: 0, y: 0 });
  });
});

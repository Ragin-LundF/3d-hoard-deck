import { describe, expect, it } from "vitest";
import { clamp, fitted, wheelFactor, zoomAt } from "./zoom";

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

  it("keeps the point under the cursor in place", () => {
    const zoomed = zoomAt({ zoom: 1, x: 0, y: 0 }, 2, { x: 100, y: -50 });
    expect(zoomed).toEqual({ zoom: 2, x: -100, y: 50 });
    // The content point under the cursor before and after: (at - offset) / zoom.
    expect((100 - zoomed.x) / zoomed.zoom).toBe(100);
  });

  it("zooms gently for trackpad steps", () => {
    expect(wheelFactor(4, false)).toBeGreaterThan(0.99);
    expect(wheelFactor(-100, false)).toBeCloseTo(1.16, 2);
  });
});

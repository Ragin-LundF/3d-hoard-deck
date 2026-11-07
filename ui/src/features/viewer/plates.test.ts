import { describe, expect, it } from "vitest";
import { decodePlates } from "./plates";

describe("decodePlates", () => {
  it("reads the plate binary written by the shell", () => {
    const buffer = new ArrayBuffer(4 + 2 * 8 + 4 * 4);
    const view = new DataView(buffer);
    view.setUint32(0, 2, true);
    view.setUint32(4, 1, true);
    view.setUint32(8, 3, true);
    [1.5, 2.5, 3.5].forEach((value, i) => view.setFloat32(12 + i * 4, value, true));
    view.setUint32(24, 4, true);
    view.setUint32(28, 1, true);
    view.setFloat32(32, -1, true);
    const plates = decodePlates(buffer);
    expect(plates.map((plate) => plate.number)).toEqual([1, 4]);
    expect([...(plates[0]?.positions ?? [])]).toEqual([1.5, 2.5, 3.5]);
    expect([...(plates[1]?.positions ?? [])]).toEqual([-1]);
  });
});

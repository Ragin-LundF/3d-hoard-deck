import { describe, expect, it } from "vitest";
import { clampMenu, suppressNativeMenu, tidy, type MenuItem } from "./context-menu";

const item = (label: string, hidden = false): MenuItem => ({ label, onSelect: () => {}, hidden });

describe("context menus", () => {
  it("drop hidden items and stray separators", () => {
    const shown = tidy(["separator", item("Open"), "separator", item("Gone", true), "separator", item("Copy path"), "separator"]);
    expect(shown.map((entry) => (entry === "separator" ? "-" : "label" in entry ? entry.label : "hint"))).toEqual(["Open", "-", "Copy path"]);
  });

  it("stay inside the window", () => {
    expect(clampMenu(990, 790, 240, 200, 1000, 800)).toEqual({ x: 754, y: 594 });
    expect(clampMenu(10, 10, 240, 200, 1000, 800)).toEqual({ x: 10, y: 10 });
  });

  it("keep the webview's own menu away except in text fields, and in development with Shift", () => {
    // Stand-ins for DOM elements: `closest` finds the text field the click is in, if any.
    const inside = (field: object | null) => ({ closest: () => field }) as unknown as EventTarget;
    const img = inside(null);
    const text = inside({ tagName: "INPUT", type: "text" });
    const box = inside({ tagName: "INPUT", type: "checkbox" });
    const area = inside({ tagName: "TEXTAREA" });
    const editable = inside({ tagName: "DIV" });
    expect([img, box, null].map((target) => suppressNativeMenu(target, false, false))).toEqual([true, true, true]);
    expect([text, area, editable].map((target) => suppressNativeMenu(target, false, false))).toEqual([false, false, false]);
    expect(suppressNativeMenu(img, true, true)).toBe(false);
    expect(suppressNativeMenu(img, true, false)).toBe(true);
  });
});

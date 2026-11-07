import { describe, expect, it } from "vitest";
import { de } from "./i18n-de";

const STRING = String.raw`"((?:[^"\\]|\\.)*)"`;

/** The app's sources as text, without tests, generated bindings and the dictionary itself. */
const files: Record<string, string> = import.meta.glob(["../**/*.{ts,tsx}", "!../**/*.test.*", "!../ipc/bindings.ts", "!./i18n-de.ts"], {
  query: "?raw",
  import: "default",
  eager: true,
});

/** Every literal passed to `t("…")` or `plural(n, "…", "…")`. */
function keys(source: string): string[] {
  const single = [...source.matchAll(new RegExp(String.raw`(?<![\w.])t\(\s*${STRING}`, "g"))].map((m) => m[1]);
  const plurals = [...source.matchAll(new RegExp(String.raw`(?<![\w.])plural\([^,]+,\s*${STRING},\s*${STRING}`, "g"))]
    .flatMap((m) => [m[1], m[2]]);
  return [...single, ...plurals].map((key) => JSON.parse(`"${key}"`) as string);
}

describe("German texts", () => {
  const used = new Set(Object.values(files).flatMap(keys));

  it("cover every text of the app", () => {
    expect(used.size).toBeGreaterThan(200);
    expect([...used].filter((key) => !(key in de))).toEqual([]);
  });

  it("keep every placeholder", () => {
    const broken = Object.entries(de).filter(([english, german]) => {
      const names = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join();
      return names(english) !== names(german);
    });
    expect(broken).toEqual([]);
  });

  it("has no leftovers", () => {
    expect(Object.keys(de).filter((key) => !used.has(key))).toEqual([]);
  });
});

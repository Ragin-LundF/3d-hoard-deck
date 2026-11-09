import { describe, expect, it } from "vitest";
import { formatRoute, parseRoute, type Route } from "./route";

describe("route", () => {
  it("round-trips folders with spaces, slashes and umlauts", () => {
    const route: Route = { view: "library", library: 2, folder: "Miniatures/Wild Life/Würfel & Co", all: true, q: "type:stl" };
    expect(parseRoute(formatRoute(route))).toEqual(route);
  });

  it("round-trips the tag browser", () => {
    expect(parseRoute(formatRoute({ view: "tags", library: 3 }))).toEqual({ view: "tags", library: 3 });
  });

  it("round-trips settings pages and falls back to the overview", () => {
    const route: Route = { view: "settings", tab: "library", lib: 2, sub: "rules" };
    expect(parseRoute(formatRoute(route))).toEqual(route);
    expect(parseRoute("#view=settings&tab=nope")).toEqual({ view: "settings", tab: "overview", lib: null, sub: "" });
  });

  it("round-trips the AI tag review", () => {
    const route: Route = { view: "review", library: 1, tab: "tag", tag: "sci fi", folder: "Miniatures/Loot" };
    expect(parseRoute(formatRoute(route))).toEqual(route);
  });

  it("round-trips collections", () => {
    expect(parseRoute(formatRoute({ view: "collection", id: 7 }))).toEqual({ view: "collection", id: 7 });
  });

  it("falls back to the library home for unknown or broken hashes", () => {
    expect(parseRoute("")).toEqual({ view: "library", library: null, folder: "", all: false, q: "" });
    expect(parseRoute("#view=model&id=abc").view).toBe("library");
  });
});

import { describe, expect, it } from "vitest";
import { formatRoute, parseRoute, type Route } from "./route";

describe("route", () => {
  it("round-trips folders with spaces, slashes and umlauts", () => {
    const route: Route = { view: "library", library: 2, folder: "Miniatures/Wild Life/Würfel & Co", all: true, q: "type:stl" };
    expect(parseRoute(formatRoute(route))).toEqual(route);
  });

  it("falls back to the library home for unknown or broken hashes", () => {
    expect(parseRoute("")).toEqual({ view: "library", library: null, folder: "", all: false, q: "" });
    expect(parseRoute("#view=model&id=abc").view).toBe("library");
  });
});

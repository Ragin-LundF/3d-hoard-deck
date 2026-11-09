import { useCallback, useSyncExternalStore } from "react";

/** Where the user is. Lives in the URL hash, so back/forward and deep links work. */
export type Route =
  | { view: "library"; library: number | null; folder: string; all: boolean; q: string }
  | { view: "model"; id: number }
  | { view: "collection"; id: number }
  | { view: "duplicates" }
  | { view: "tags"; library: number }
  | { view: "review"; library: number; tab: ReviewTab; tag: string; folder: string }
  | { view: "settings"; tab: SettingsTab; lib: number | null; sub: string };

/** The pages of Settings; `library` is one library's detail (`lib`), `sub` a tab inside a page. */
export const SETTINGS_TABS = ["overview", "libraries", "library", "files", "tags", "ai", "slicers", "general", "index", "updates"] as const;
export type SettingsTab = (typeof SETTINGS_TABS)[number];
export type SettingsRoute = Extract<Route, { view: "settings" }>;

/** The AI tag review: by tag (main view), by model, what was reviewed, and the folders the AI works on. */
export const REVIEW_TABS = ["tag", "model", "reviewed", "folders"] as const;
export type ReviewTab = (typeof REVIEW_TABS)[number];
export type ReviewRoute = Extract<Route, { view: "review" }>;

export const reviewRoute = (library: number, tab: ReviewTab = "tag", tag = "", folder = ""): ReviewRoute => ({ view: "review", library, tab, tag, folder });

export const settingsRoute = (tab: SettingsTab = "overview", lib: number | null = null, sub = ""): SettingsRoute => ({ view: "settings", tab, lib, sub });

export type LibraryRoute = Extract<Route, { view: "library" }>;

export const HOME: LibraryRoute = { view: "library", library: null, folder: "", all: false, q: "" };

export function parseRoute(hash: string): Route {
  const params = new URLSearchParams(hash.replace(/^#/, ""));
  const view = params.get("view");
  if (view === "settings") {
    const tab = SETTINGS_TABS.find((name) => name === params.get("tab")) ?? "overview";
    const lib = Number(params.get("lib"));
    return settingsRoute(tab, Number.isInteger(lib) && lib > 0 ? lib : null, params.get("sub") ?? "");
  }
  if (view === "duplicates") return { view: "duplicates" };
  const tagsLibrary = Number(params.get("lib"));
  if (view === "review" && Number.isInteger(tagsLibrary) && tagsLibrary > 0) {
    const tab = REVIEW_TABS.find((name) => name === params.get("tab")) ?? "tag";
    return reviewRoute(tagsLibrary, tab, params.get("tag") ?? "", params.get("folder") ?? "");
  }
  if (view === "tags" && Number.isInteger(tagsLibrary) && tagsLibrary > 0) return { view: "tags", library: tagsLibrary };
  const id = Number(params.get("id"));
  if (view === "model" && Number.isInteger(id) && id > 0) return { view: "model", id };
  if (view === "collection" && Number.isInteger(id) && id > 0) return { view: "collection", id };
  const library = Number(params.get("lib"));
  return {
    view: "library",
    library: Number.isInteger(library) && library > 0 ? library : null,
    folder: params.get("folder") ?? "",
    all: params.get("all") === "1",
    q: params.get("q") ?? "",
  };
}

export function formatRoute(route: Route): string {
  const params = new URLSearchParams({ view: route.view });
  if (route.view === "model" || route.view === "collection") params.set("id", String(route.id));
  if (route.view === "tags") params.set("lib", String(route.library));
  if (route.view === "review") {
    params.set("lib", String(route.library));
    if (route.tab !== "tag") params.set("tab", route.tab);
    if (route.tag) params.set("tag", route.tag);
    if (route.folder) params.set("folder", route.folder);
  }
  if (route.view === "settings") {
    if (route.tab !== "overview") params.set("tab", route.tab);
    if (route.lib !== null) params.set("lib", String(route.lib));
    if (route.sub) params.set("sub", route.sub);
  }
  if (route.view === "library") {
    if (route.library !== null) params.set("lib", String(route.library));
    if (route.folder) params.set("folder", route.folder);
    if (route.all) params.set("all", "1");
    if (route.q) params.set("q", route.q);
  }
  return `#${params.toString()}`;
}

function subscribe(onChange: () => void): () => void {
  window.addEventListener("hashchange", onChange);
  return () => window.removeEventListener("hashchange", onChange);
}

export function useRoute(): [Route, (route: Route, options?: { replace?: boolean }) => void] {
  const hash = useSyncExternalStore(subscribe, () => window.location.hash);
  const navigate = useCallback((route: Route, options?: { replace?: boolean }) => {
    const next = formatRoute(route);
    if (options?.replace) {
      history.replaceState(null, "", next);
      window.dispatchEvent(new HashChangeEvent("hashchange"));
    } else {
      window.location.hash = next;
    }
  }, []);
  return [parseRoute(hash), navigate];
}

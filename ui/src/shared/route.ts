import { useCallback, useSyncExternalStore } from "react";

/** Where the user is. Lives in the URL hash, so back/forward and deep links work. */
export type Route =
  | { view: "library"; library: number | null; folder: string; all: boolean; q: string }
  | { view: "model"; id: number }
  | { view: "settings" };

export type LibraryRoute = Extract<Route, { view: "library" }>;

export const HOME: LibraryRoute = { view: "library", library: null, folder: "", all: false, q: "" };

export function parseRoute(hash: string): Route {
  const params = new URLSearchParams(hash.replace(/^#/, ""));
  const view = params.get("view");
  if (view === "settings") return { view: "settings" };
  const id = Number(params.get("id"));
  if (view === "model" && Number.isInteger(id) && id > 0) return { view: "model", id };
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
  if (route.view === "model") params.set("id", String(route.id));
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

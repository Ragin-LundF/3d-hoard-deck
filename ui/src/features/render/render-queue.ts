import { useEffect, useSyncExternalStore } from "react";
import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { commands, unwrap } from "../../ipc";
import { viewerFormat } from "../viewer/formats";

/** Meshes above this are not rendered by themselves; the card offers "Render" instead. */
export const AUTO_LIMIT = 50 * 1024 * 1024;
const MODE_KEY = "renderMode";

export type RenderMode = "auto" | "request" | "off";
export const renderMode = (): RenderMode => {
  const stored = localStorage.getItem(MODE_KEY);
  return stored === "request" || stored === "off" ? stored : "auto";
};
export const setRenderMode = (mode: RenderMode) => localStorage.setItem(MODE_KEY, mode);

type Job = { library: number; id: number; manual: boolean };
let queue: Job[] = [];
let running: number | null = null;
let client: QueryClient | null = null;
/** Models that could not be rendered by themselves (too big, unreadable), so they are not tried again this session. */
const failed = new Map<number, "big" | "error">();
const listeners = new Set<() => void>();
const emit = () => listeners.forEach((listener) => listener());

/** For a card: being rendered, too big to render by itself, could not be rendered, or nothing. */
export function useRenderState(id: number): "busy" | "big" | "error" | "" {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => (running === id ? "busy" : failed.get(id) ?? ""),
  );
}

/** A card that came into view asks for a render; `manual` (a Render click) also renders big or failed models. */
export function requestRender(library: number, id: number, manual = false) {
  if (!manual && failed.has(id)) return;
  if (manual) failed.delete(id);
  if (running === id || queue.some((job) => job.id === id)) return;
  queue.push({ library, id, manual });
  void work();
}

/** A card scrolled away before its turn: no need to render it now. */
export function cancelRender(id: number) {
  queue = queue.filter((job) => job.id !== id || job.manual);
}

async function work() {
  if (running !== null) return;
  const job = queue.shift();
  if (!job) return;
  running = job.id;
  emit();
  try {
    const source = await unwrap(commands.renderSource(job.id));
    const format = source && viewerFormat(source.path.split(".").pop()?.toLowerCase() ?? "");
    if (!source || !format) failed.set(job.id, "error");
    else if (!job.manual && (source.size ?? 0) > AUTO_LIMIT) failed.set(job.id, "big");
    else {
      const { renderPreview } = await import("../viewer/render-preview");
      const png = await renderPreview(job.library, source.path, format, new AbortController().signal);
      await unwrap(commands.storeRenderedPreview(job.id, Array.from(png)));
      void client?.invalidateQueries({ predicate: (query) => ["browse", "search", "model"].includes(String(query.queryKey[0])) });
    }
  } catch {
    failed.set(job.id, "error");
  }
  running = null;
  emit();
  void work();
}

/** Mounted once in the app shell, so finished renders refresh the grid. */
export function useRenderQueue() {
  const queryClient = useQueryClient();
  useEffect(() => {
    client = queryClient;
  }, [queryClient]);
}

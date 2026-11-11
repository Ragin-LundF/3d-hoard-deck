import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent as ReactMouseEvent } from "react";

/** One entry of a menu: an action, a line between groups, or a short explanation. */
export type MenuItem =
  | { label: string; onSelect: () => void; checked?: boolean; hidden?: boolean; danger?: boolean }
  | "separator"
  | { hint: string };

type OpenMenu = { x: number; y: number; items: MenuItem[]; returnFocus: HTMLElement | null };

let current: OpenMenu | null = null;
const listeners = new Set<() => void>();
const set = (next: OpenMenu | null) => {
  current = next;
  listeners.forEach((listener) => listener());
};

/** Opens a menu at the pointer (or below the focused element for the keyboard); only one menu is open at a time. */
export function openContextMenu(event: ReactMouseEvent | ReactKeyboardEvent | { clientX: number; clientY: number }, items: MenuItem[]) {
  let x = "clientX" in event ? event.clientX : 0;
  let y = "clientY" in event ? event.clientY : 0;
  let returnFocus: HTMLElement | null = null;
  if ("preventDefault" in event) {
    event.preventDefault();
    event.stopPropagation();
    returnFocus = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
    // From the keyboard (Context Menu key, Shift+F10) there is no pointer: open at the element.
    if ((!("clientX" in event) || (x === 0 && y === 0)) && returnFocus) {
      const box = returnFocus.getBoundingClientRect();
      x = box.left + 12;
      y = box.top + Math.min(box.height, 24);
    }
  }
  lastPoint = { x, y };
  const visible = tidy(items);
  if (visible.some((item) => typeof item === "object" && "label" in item)) set({ x, y, items: visible, returnFocus });
}

let lastPoint = { x: 0, y: 0 };

/** A follow-up menu where the last one was, e.g. the collections after "Add to collection…". */
export function openAtLastPoint(items: MenuItem[]) {
  openContextMenu({ clientX: lastPoint.x, clientY: lastPoint.y }, items);
}

export const closeContextMenu = () => set(null);

/** Right-click opens the menu `items()` builds; the Context Menu key and Shift+F10 fire the same event on the focused
 * element. */
export function menuProps(items: () => MenuItem[]) {
  return { onContextMenu: (event: ReactMouseEvent) => openContextMenu(event, items()) };
}

/** Hidden items out; no separator at the start, the end, or twice in a row. */
export function tidy(items: MenuItem[]): MenuItem[] {
  const shown = items.filter((item) => !(typeof item === "object" && "label" in item && item.hidden));
  const result: MenuItem[] = [];
  for (const item of shown) {
    if (item === "separator" && (result.length === 0 || result[result.length - 1] === "separator")) continue;
    result.push(item);
  }
  while (result[result.length - 1] === "separator") result.pop();
  return result;
}

/** Keeps a menu of `width` × `height` at (`x`, `y`) inside a window of `viewWidth` × `viewHeight`. */
export function clampMenu(x: number, y: number, width: number, height: number, viewWidth: number, viewHeight: number) {
  const margin = 6;
  return {
    x: Math.max(margin, Math.min(x, viewWidth - width - margin)),
    y: Math.max(margin, Math.min(y, viewHeight - height - margin)),
  };
}

/** Whether the webview's own menu ("Reload", "Save Image", …) is kept away: everywhere except in text fields, and in
 * development with Shift held (for "Inspect Element"). */
export function suppressNativeMenu(target: EventTarget | null, shiftKey: boolean, dev: boolean): boolean {
  if (dev && shiftKey) return false;
  const closest = (target as { closest?: (selector: string) => Element | null } | null)?.closest;
  if (typeof closest !== "function") return true;
  const field = closest.call(target, "textarea, [contenteditable=''], [contenteditable='true'], input");
  if (!field) return true;
  if (field.tagName === "INPUT") {
    return !["text", "search", "url", "email", "tel", "password", "number"].includes((field as HTMLInputElement).type);
  }
  return false;
}

/** Installed once at start: the app shows its own menus or none. */
export function installNativeMenuGuard() {
  window.addEventListener("contextmenu", (event) => {
    if (suppressNativeMenu(event.target, event.shiftKey, import.meta.env.DEV)) event.preventDefault();
  });
}

/** The one open menu, rendered by the app shell. */
export function ContextMenuHost() {
  const menu = useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => current,
  );
  const ref = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState<{ x: number; y: number } | null>(null);

  useLayoutEffect(() => {
    if (!menu || !ref.current) return setPlace(null);
    const box = ref.current.getBoundingClientRect();
    setPlace(clampMenu(menu.x, menu.y, box.width, box.height, window.innerWidth, window.innerHeight));
    ref.current.querySelector<HTMLButtonElement>("button")?.focus();
  }, [menu]);

  useEffect(() => {
    if (!menu) return;
    const close = () => set(null);
    const outside = (event: MouseEvent) => {
      if (!ref.current?.contains(event.target as Node)) close();
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        menu.returnFocus?.focus();
        close();
      }
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        const buttons = [...(ref.current?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
        const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
        const next = event.key === "ArrowDown" ? (at + 1) % buttons.length : (at - 1 + buttons.length) % buttons.length;
        buttons[next]?.focus();
      }
    };
    window.addEventListener("mousedown", outside, true);
    window.addEventListener("keydown", key);
    window.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("mousedown", outside, true);
      window.removeEventListener("keydown", key);
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", close);
      window.removeEventListener("blur", close);
    };
  }, [menu]);

  if (!menu) return null;
  return (
    <div
      ref={ref}
      className="menu"
      role="menu"
      style={{ left: place?.x ?? menu.x, top: place?.y ?? menu.y, visibility: place ? "visible" : "hidden" }}
      onContextMenu={(event) => event.preventDefault()}
    >
      {menu.items.map((item, index) => {
        if (item === "separator") return <hr key={index} />;
        if ("hint" in item) return <p key={index} className="menu-hint">{item.hint}</p>;
        return (
          <button
            key={index}
            role={item.checked === undefined ? "menuitem" : "menuitemcheckbox"}
            aria-checked={item.checked}
            className={item.danger ? "danger" : undefined}
            onClick={() => {
              set(null);
              item.onSelect();
            }}
          >
            {item.label}
          </button>
        );
      })}
    </div>
  );
}

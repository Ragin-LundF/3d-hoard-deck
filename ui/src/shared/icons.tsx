import type { ReactElement } from "react";

const PATHS: Record<string, ReactElement> = {
  search: <><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></>,
  folder: <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />,
  grid: <><rect x="3" y="3" width="7" height="7" rx="1.5" /><rect x="14" y="3" width="7" height="7" rx="1.5" /><rect x="3" y="14" width="7" height="7" rx="1.5" /><rect x="14" y="14" width="7" height="7" rx="1.5" /></>,
  layers: <><path d="M12 3 3 8l9 5 9-5z" /><path d="m3 13 9 5 9-5" /></>,
  refresh: <><path d="M21 12a9 9 0 1 1-2.6-6.4" /><path d="M21 4v5h-5" /></>,
  settings: <><circle cx="12" cy="12" r="3" /><path d="M12 2v3M12 19v3M4.9 4.9 7 7M17 17l2.1 2.1M2 12h3M19 12h3M4.9 19.1 7 17M17 7l2.1-2.1" /></>,
  sun: <><circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" /></>,
  moon: <path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z" />,
  back: <path d="m15 18-6-6 6-6" />,
  chevron: <path d="m9 18 6-6-6-6" />,
  drive: <><rect x="2" y="13" width="20" height="8" rx="2" /><path d="M6 17h.01M10 17h.01M4 13l2.5-8h11L20 13" /></>,
  plus: <path d="M12 5v14M5 12h14" />,
  box: <><rect x="3" y="7" width="18" height="13" rx="2" /><path d="M7 7V5h10v2" /></>,
  close: <path d="m6 6 12 12M18 6 6 18" />,
};

export function Icon({ name, label }: { name: keyof typeof PATHS; label?: string }) {
  return (
    <svg className="i" viewBox="0 0 24 24" aria-hidden={label ? undefined : true} aria-label={label} role={label ? "img" : undefined}>
      {PATHS[name]}
    </svg>
  );
}

export function Logo() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M4 18h16l-2 3H6z" fill="#FF7A45" />
      <path d="M7 15h10l-1.2 2H8.2z" fill="#FF7A45" opacity=".7" />
      <path d="M12 3l5 9H7z" fill="currentColor" />
    </svg>
  );
}

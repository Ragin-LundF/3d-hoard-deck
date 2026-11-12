import { useState, useSyncExternalStore } from "react";
import { formatNumber, t } from "../../shared/i18n";
import { Icon } from "../../shared/icons";
import { useTags } from "./queries";
import { menuProps } from "../../shared/context-menu";
import { useTagMenu } from "./tag-menu";
import { toggleExclude, toggleInclude } from "./tag-query";

const SHOW_KEY = "showPinnedTags";
const COLLAPSED_KEY = "pinnedTagsCollapsed";

/** Settings › Tags: whether pinned tags appear in the sidebar (default on). */
export function showPinnedTags(): boolean {
  return localStorage.getItem(SHOW_KEY) !== "false";
}

const CHANGED = "pinned-tags-setting";

export function setShowPinnedTags(show: boolean) {
  localStorage.setItem(SHOW_KEY, String(show));
  window.dispatchEvent(new Event(CHANGED));
}

/** The setting, re-read when Settings changes it. */
export function useShowPinnedTags(): boolean {
  return useSyncExternalStore((onChange) => {
    window.addEventListener(CHANGED, onChange);
    return () => window.removeEventListener(CHANGED, onChange);
  }, showPinnedTags);
}

type Props = { library: number; query: string | null; onSearch: (query: string) => void; onMessage: (message: string) => void };

/** The few tags the user pinned, below the folders, collapsible. Nothing shows until a tag is pinned. */
export function PinnedTags({ library, query, onSearch, onMessage }: Props) {
  const tags = useTags(library);
  const tagMenu = useTagMenu(onMessage);
  const show = useShowPinnedTags();
  const [collapsed, setCollapsed] = useState(() => localStorage.getItem(COLLAPSED_KEY) === "true");
  const pinned = (tags.data ?? []).filter((tag) => tag.pinned);
  if (!show || pinned.length === 0) return null;
  const toggle = () => {
    localStorage.setItem(COLLAPSED_KEY, String(!collapsed));
    setCollapsed(!collapsed);
  };
  return (
    <>
      <button className="subhead with-action" onClick={toggle} aria-expanded={!collapsed}>
        <Icon name="chevron" />{t("Pinned tags")}
      </button>
      {!collapsed &&
        pinned.map((tag) => {
          const active = query !== null && query.toLowerCase().split(/\s+/).includes(`tag:${tag.name.toLowerCase()}`);
          return (
            <button key={tag.name} className={`nav${active ? " on" : ""}`} onClick={() => onSearch(toggleInclude(query ?? "", tag.name))}
              {...menuProps(() => tagMenu(library, tag.name, { onShow: () => onSearch(toggleInclude("", tag.name)), onExclude: () => onSearch(toggleExclude("", tag.name)) }))}>
              <Icon name="tag" /><span className="lbl">{tag.name}</span><span className="n">{formatNumber(tag.count)}</span>
            </button>
          );
        })}
    </>
  );
}

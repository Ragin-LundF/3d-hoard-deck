import { useState, type MouseEvent } from "react";
import { createPortal } from "react-dom";
import { formatNumber, t } from "../../shared/i18n";
import { Icon } from "../../shared/icons";
import { AllTags } from "./all-tags";
import { useTagFacets } from "./queries";
import { modeOf, readTags, setMode, toggleExclude, toggleInclude, wanted } from "./tag-query";

/** Chips shown before "All tags…". */
const CHIPS = 14;

type Props = {
  library: number;
  folder: string;
  query: string;
  onQuery: (query: string) => void;
  /** Opens "Check this tag" for an active tag. */
  onCheck?: (tag: string) => void;
  /** Removes an active tag from every model shown. */
  onRemoveAll?: (tag: string) => void;
};

/** The most common tags of what is shown, as chips above the grid. Click requires a tag, ⌥-click excludes it. */
export function TagBar({ library, folder, query, onQuery, onCheck, onRemoveAll }: Props) {
  const filter = readTags(query);
  const facets = useTagFacets(library, folder, query, CHIPS + 10);
  const [open, setOpen] = useState(false);
  // The tag row clips its chips, so the menu is placed on the page at the button's position.
  const [menu, setMenu] = useState<{ tag: string; x: number; y: number } | null>(null);
  const active = [...wanted(filter), ...filter.exclude];
  const isActive = (name: string) => active.some((tag) => tag.toLowerCase() === name.toLowerCase());
  const suggestions = (facets.data ?? []).filter((facet) => !isActive(facet.name)).slice(0, CHIPS);
  if (active.length === 0 && suggestions.length === 0) return null;
  const click = (name: string) => (event: MouseEvent) =>
    onQuery(event.altKey ? toggleExclude(query, name) : toggleInclude(query, name));

  return (
    <div className="tagbar" aria-label={t("Tags in this view")}>
      <span className="lead"><Icon name="tag" />{t("Tags")}</span>
      {wanted(filter).length > 1 && (
        <div className="seg mini" role="group" aria-label={t("Match")}>
          <button className={modeOf(filter) === "all" ? "on" : ""} onClick={() => onQuery(setMode(query, "all"))} title={t("Models with all of these tags")}>{t("All")}</button>
          <button className={modeOf(filter) === "any" ? "on" : ""} onClick={() => onQuery(setMode(query, "any"))} title={t("Models with at least one of these tags")}>{t("Any")}</button>
        </div>
      )}
      <div className="track">
        {wanted(filter).map((tag) => (
          <span key={`+${tag}`} className="split chip-split">
            <button className="chip inc" onClick={click(tag)} title={t("Click to remove")} aria-pressed="true">
              {tag}<Icon name="close" />
            </button>
            {(onCheck || onRemoveAll) && (
              <button className="chip inc menu-btn" aria-haspopup="menu" aria-expanded={menu?.tag === tag} aria-label={t("More for {tag}", { tag })}
                onClick={(event) => {
                  const box = event.currentTarget.getBoundingClientRect();
                  setMenu(menu?.tag === tag ? null : { tag, x: box.left, y: box.bottom + 4 });
                }}>▾</button>
            )}
            {menu?.tag === tag && createPortal(
              // On the page itself: the tag row's fade mask would cut the menu off.
              <div className="menu" role="menu" style={{ left: menu.x, top: menu.y }} onMouseLeave={() => setMenu(null)}>
                {onCheck && <button role="menuitem" onClick={() => { setMenu(null); onCheck(tag); }}>{t("Check this tag…")}</button>}
                {onRemoveAll && <button role="menuitem" onClick={() => { setMenu(null); onRemoveAll(tag); }}>{t("Remove from all here…")}</button>}
              </div>,
              document.body,
            )}
          </span>
        ))}
        {filter.exclude.map((tag) => (
          <button key={`-${tag}`} className="chip exc" onClick={() => onQuery(toggleExclude(query, tag))} title={t("Excluded. Click to remove")} aria-pressed="true">
            {tag}<Icon name="close" />
          </button>
        ))}
        {suggestions.map((facet) => (
          <button key={facet.name} className="chip" onClick={click(facet.name)} title={t("Click: only models with this tag. ⌥-click: without it")}>
            {facet.name}<span className="n">{formatNumber(facet.count)}</span>
          </button>
        ))}
      </div>
      <div className="split">
        <button className="chip more" onClick={() => setOpen(!open)} aria-haspopup="dialog" aria-expanded={open}>{t("All tags…")}</button>
        {open && (
          <AllTags
            library={library}
            folder={folder}
            query={query}
            onPick={(name, exclude) => onQuery(exclude ? toggleExclude(query, name) : toggleInclude(query, name))}
            onClose={() => setOpen(false)}
          />
        )}
      </div>
    </div>
  );
}

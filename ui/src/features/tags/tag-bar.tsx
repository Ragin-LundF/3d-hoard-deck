import { useState, type MouseEvent } from "react";
import { menuProps, openContextMenu } from "../../shared/context-menu";
import { formatNumber, t } from "../../shared/i18n";
import { Icon } from "../../shared/icons";
import { AllTags } from "./all-tags";
import { useTagFacets } from "./queries";
import { useTagMenu } from "./tag-menu";
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
  onMessage?: (message: string) => void;
};

/** The most common tags of what is shown, as chips above the grid. Click requires a tag, ⌥-click excludes it. */
export function TagBar({ library, folder, query, onQuery, onCheck, onRemoveAll, onMessage = () => {} }: Props) {
  const tagMenu = useTagMenu(onMessage);
  const filter = readTags(query);
  const facets = useTagFacets(library, folder, query, CHIPS + 10);
  const [open, setOpen] = useState(false);
  const active = [...wanted(filter), ...filter.exclude];
  const isActive = (name: string) => active.some((tag) => tag.toLowerCase() === name.toLowerCase());
  const suggestions = (facets.data ?? []).filter((facet) => !isActive(facet.name)).slice(0, CHIPS);
  if (active.length === 0 && suggestions.length === 0) return null;
  const click = (name: string) => (event: MouseEvent) =>
    onQuery(event.altKey ? toggleExclude(query, name) : toggleInclude(query, name));
  /** The same menu for a right-click on a chip and its "▾"; "Remove from all here" only for a tag in use here. */
  const menuOf = (name: string, active: boolean) =>
    tagMenu(library, name, {
      onShow: () => onQuery(isActive(name) ? query : toggleInclude(query, name)),
      onExclude: () => onQuery(toggleExclude(isActive(name) ? toggleInclude(query, name) : query, name)),
      onCheck: active && onCheck ? () => onCheck(name) : undefined,
      onRemoveAll: active && onRemoveAll ? () => onRemoveAll(name) : undefined,
    });

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
            <button className="chip inc" onClick={click(tag)} title={t("Click to remove")} aria-pressed="true" {...menuProps(() => menuOf(tag, true))}>
              {tag}<Icon name="close" />
            </button>
            {(onCheck || onRemoveAll) && (
              <button className="chip inc menu-btn" aria-haspopup="menu" aria-label={t("More for {tag}", { tag })}
                onClick={(event) => {
                  const box = event.currentTarget.getBoundingClientRect();
                  openContextMenu({ clientX: box.left, clientY: box.bottom + 4 }, menuOf(tag, true));
                }}>▾</button>
            )}
          </span>
        ))}
        {filter.exclude.map((tag) => (
          <button key={`-${tag}`} className="chip exc" onClick={() => onQuery(toggleExclude(query, tag))} title={t("Excluded. Click to remove")} aria-pressed="true" {...menuProps(() => menuOf(tag, false))}>
            {tag}<Icon name="close" />
          </button>
        ))}
        {suggestions.map((facet) => (
          <button key={facet.name} className="chip" onClick={click(facet.name)} title={t("Click: only models with this tag. ⌥-click: without it")} {...menuProps(() => menuOf(facet.name, false))}>
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

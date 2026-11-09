import { useEffect, useMemo, useRef, useState } from "react";
import { formatNumber, t } from "../../shared/i18n";
import { useTagFacets, useTagGroups, useTags } from "./queries";

/** Matches listed at most; typing narrows the list. */
const SHOWN = 120;

type Props = {
  library: number;
  folder: string;
  query: string;
  onPick: (name: string, exclude: boolean) => void;
  onClose: () => void;
};

/** Searchable list of every tag: those in the current view first, then the rest. Keyboard first. */
export function AllTags({ library, folder, query, onPick, onClose }: Props) {
  const [text, setText] = useState("");
  const [group, setGroup] = useState<number | null>(null);
  const [cursor, setCursor] = useState(0);
  const panel = useRef<HTMLDivElement>(null);
  const tags = useTags(library);
  const groups = useTagGroups(library);
  const inView = useTagFacets(library, folder, query, 500);

  const rows = useMemo(() => {
    const needle = text.trim().toLowerCase();
    const counts = new Map((inView.data ?? []).map((facet) => [facet.name.toLowerCase(), facet.count]));
    const matching = (tags.data ?? []).filter(
      (tag) => (!needle || tag.name.toLowerCase().includes(needle)) && (group === null || tag.group === group),
    );
    const here = matching.filter((tag) => counts.has(tag.name.toLowerCase()));
    const elsewhere = matching.filter((tag) => !counts.has(tag.name.toLowerCase()));
    return [
      ...here.map((tag) => ({ name: tag.name, count: counts.get(tag.name.toLowerCase()) ?? 0, here: true })),
      ...elsewhere.map((tag) => ({ name: tag.name, count: tag.count, here: false })),
    ].slice(0, SHOWN);
  }, [tags.data, inView.data, text, group]);

  useEffect(() => setCursor(0), [text, group]);
  useEffect(() => {
    const close = (event: globalThis.MouseEvent) => {
      if (panel.current && !panel.current.contains(event.target as Node)) onClose();
    };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [onClose]);

  const pick = (name: string, exclude: boolean) => {
    onPick(name, exclude);
    onClose();
  };
  const firstElsewhere = rows.findIndex((row) => !row.here);

  return (
    <div className="pop" role="dialog" aria-label={t("All tags")} ref={panel}>
      <div className="ph">
        <input
          autoFocus
          value={text}
          placeholder={t("Find a tag…")}
          aria-label={t("Find a tag…")}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") onClose();
            if (event.key === "ArrowDown") setCursor((at) => Math.min(rows.length - 1, at + 1));
            if (event.key === "ArrowUp") setCursor((at) => Math.max(0, at - 1));
            const row = rows[cursor];
            if (event.key === "Enter" && row) pick(row.name, event.altKey);
          }}
        />
      </div>
      {(groups.data?.length ?? 0) > 0 && (
        <div className="groups">
          <button className={`chip${group === null ? " inc" : ""}`} onClick={() => setGroup(null)}>{t("All groups")}</button>
          {groups.data?.map((item) => (
            <button key={item.id} className={`chip${group === item.id ? " inc" : ""}`} onClick={() => setGroup(group === item.id ? null : (item.id ?? null))}>
              <span className="gd" style={{ background: item.color }} />{item.name}
            </button>
          ))}
        </div>
      )}
      <div className="body" role="listbox" aria-label={t("Tags")}>
        {rows.length === 0 && <p className="hint">{t("No tag matches.")}</p>}
        {rows[0]?.here && <h5>{t("In this view")}</h5>}
        {rows.map((row, index) => (
          <div key={row.name}>
            {index === firstElsewhere && <h5>{t("Elsewhere")}</h5>}
            <button
              role="option"
              aria-selected={index === cursor}
              className={`poprow${index === cursor ? " on" : ""}`}
              onMouseEnter={() => setCursor(index)}
              onClick={(event) => pick(row.name, event.altKey)}
            >
              <span>{row.name}</span>
              <span className="n">{formatNumber(row.count)}</span>
            </button>
          </div>
        ))}
      </div>
      <div className="pf">
        <span><kbd>↵</kbd> {t("only with")}</span>
        <span><kbd>⌥↵</kbd> {t("without")}</span>
        <span><kbd>Esc</kbd> {t("close")}</span>
      </div>
    </div>
  );
}

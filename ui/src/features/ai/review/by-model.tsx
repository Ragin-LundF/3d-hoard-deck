import { useState } from "react";
import { thumbUrl, type ReviewItemDto } from "../../../ipc";
import { Icon } from "../../../shared/icons";
import { plural, t } from "../../../shared/i18n";
import { useDecide } from "./queries";
import type { LogEntry } from "./review-log";

type Props = { library: number; items: ReviewItemDto[]; onDone: (entry: LogEntry) => void; onMessage: (message: string) => void };

/** All suggestions of each model: click a chip to drop it, then keep the rest. Several models can be selected. */
export function ByModel({ library, items, onDone, onMessage }: Props) {
  const [dropped, setDropped] = useState<Record<number, Set<string>>>({});
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const decide = useDecide(library, onMessage);
  if (items.length === 0) return <div className="rv-empty">{t("All suggestions are reviewed.")}</div>;
  const dropsOf = (item: ReviewItemDto) => dropped[item.id] ?? new Set<string>();
  const toggleDrop = (item: ReviewItemDto, tag: string) => {
    const next = new Set(dropsOf(item));
    if (next.has(tag)) next.delete(tag);
    else next.add(tag);
    setDropped({ ...dropped, [item.id]: next });
  };
  const decisionFor = (item: ReviewItemDto, dismissAll: boolean) => ({
    id: item.id,
    keep: dismissAll ? [] : item.suggested.filter((tag) => !dropsOf(item).has(tag)),
    dismiss: dismissAll ? item.suggested : item.suggested.filter((tag) => dropsOf(item).has(tag)),
  });
  const run = (targets: ReviewItemDto[], dismissAll: boolean) => {
    const label = dismissAll
      ? plural(targets.length, "Dismissed the suggestions of {count} model", "Dismissed the suggestions of {count} models")
      : plural(targets.length, "Kept suggestions on {count} model", "Kept suggestions on {count} models");
    decide.mutate({ decisions: targets.map((item) => decisionFor(item, dismissAll)), label }, {
      onSuccess: (entry) => {
        setSelected(new Set());
        onDone(entry);
      },
    });
  };
  const chosen = items.filter((item) => selected.has(item.id));

  return (
    <section className="rv-main">
      <div className="rv-bar">
        <label className="tr-check">
          <input type="checkbox" checked={chosen.length === items.length} onChange={(event) => setSelected(event.target.checked ? new Set(items.map((item) => item.id)) : new Set())} />
          {t("Select all")}
        </label>
        <span className="spacer" />
        {chosen.length > 0 && (
          <>
            <button className="btn primary small" disabled={decide.isPending} onClick={() => run(chosen, false)}>{plural(chosen.length, "Keep tags on {count} model", "Keep tags on {count} models")}</button>
            <button className="btn ghost small" disabled={decide.isPending} onClick={() => run(chosen, true)}>{t("Dismiss all")}</button>
          </>
        )}
      </div>
      <div className="rv-cards">
        {items.map((item) => (
          <div key={item.id} className={`rv-card${selected.has(item.id) ? " sel" : ""}`}>
            <label className="cb">
              <input type="checkbox" checked={selected.has(item.id)} aria-label={t("Select {name}", { name: item.title })}
                onChange={(event) => { const next = new Set(selected); if (event.target.checked) next.add(item.id); else next.delete(item.id); setSelected(next); }} />
            </label>
            <div className="cover">{item.cover ? <img src={thumbUrl(library, item.cover)} alt="" loading="lazy" /> : <span className="none">{t("No preview")}</span>}</div>
            <div className="name" title={item.path}>{item.title}</div>
            {item.own.length > 0 && <div className="also">{t("Has: {tags}", { tags: item.own.join(", ") })}</div>}
            <div className="rv-chips">
              {item.suggested.map((tag) => {
                const drop = dropsOf(item).has(tag);
                return (
                  <button key={tag} className={`chip auto ai${drop ? " dropped" : ""}`} aria-pressed={!drop} onClick={() => toggleDrop(item, tag)} title={drop ? t("Keep {tag}", { tag }) : t("Drop {tag}", { tag })}>
                    {tag}
                  </button>
                );
              })}
            </div>
            <div className="acts">
              <button className="btn small" disabled={decide.isPending} onClick={() => run([item], false)}><Icon name="check" />{t("Keep the rest")}</button>
              <button className="btn ghost small" disabled={decide.isPending} onClick={() => run([item], true)}>{t("Dismiss")}</button>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

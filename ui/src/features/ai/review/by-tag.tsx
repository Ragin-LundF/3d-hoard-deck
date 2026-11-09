import { useEffect, useMemo, useState } from "react";
import { thumbUrl, type ReviewItemDto } from "../../../ipc";
import { formatNumber, plural, t } from "../../../shared/i18n";
import { useDecide } from "./queries";
import type { LogEntry } from "./review-log";

type Props = { library: number; items: ReviewItemDto[]; tag: string; onTag: (tag: string) => void; onDone: (entry: LogEntry) => void; onMessage: (message: string) => void };

const key = (tag: string) => tag.toLocaleLowerCase();

/**
 * The main view: one suggested tag at a time, with every model it was suggested for. Switch the tag off on the
 * misses, then keep it on the rest in one go; the next tag opens.
 */
export function ByTag({ library, items, tag, onTag, onDone, onMessage }: Props) {
  const [filter, setFilter] = useState("");
  const [off, setOff] = useState<Record<string, Set<number>>>({});
  const [anchor, setAnchor] = useState<number | null>(null);
  const decide = useDecide(library, onMessage);
  const tags = useMemo(() => {
    const counts = new Map<string, { name: string; count: number }>();
    for (const item of items) {
      for (const name of item.suggested) {
        const entry = counts.get(key(name)) ?? { name, count: 0 };
        entry.count += 1;
        counts.set(key(name), entry);
      }
    }
    return [...counts.values()].sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
  }, [items]);
  const current = tags.find((entry) => key(entry.name) === key(tag)) ?? tags[0];
  useEffect(() => {
    if (current && key(current.name) !== key(tag)) onTag(current.name);
  }, [current, tag, onTag]);
  if (!current) return <div className="rv-empty">{t("All suggestions are reviewed.")}</div>;
  const models = items.filter((item) => item.suggested.some((name) => key(name) === key(current.name)));
  const skipped = off[key(current.name)] ?? new Set<number>();
  const setSkipped = (next: Set<number>) => setOff({ ...off, [key(current.name)]: next });
  const toggle = (index: number, range: boolean) => {
    const next = new Set(skipped);
    const turnOff = !skipped.has(models[index]!.id);
    const [from, to] = range && anchor !== null ? [Math.min(anchor, index), Math.max(anchor, index)] : [index, index];
    for (let i = from; i <= to; i++) {
      if (turnOff) next.add(models[i]!.id);
      else next.delete(models[i]!.id);
    }
    setSkipped(next);
    setAnchor(index);
  };
  const resolve = (keepAll: boolean | null) => {
    const keep = (item: ReviewItemDto) => keepAll ?? !skipped.has(item.id);
    const decisions = models.map((item) => ({ id: item.id, keep: keep(item) ? [current.name] : [], dismiss: keep(item) ? [] : [current.name] }));
    const kept = decisions.filter((decision) => decision.keep.length > 0).length;
    decide.mutate(
      { decisions, label: t("{tag}: kept on {kept}, skipped {skipped}", { tag: current.name, kept, skipped: decisions.length - kept }) },
      { onSuccess: onDone },
    );
  };
  const keeping = models.length - models.filter((item) => skipped.has(item.id)).length;
  const shown = tags.filter((entry) => entry.name.toLowerCase().includes(filter.trim().toLowerCase()));

  return (
    <div className="rv-bytag">
      <aside className="rv-tags" aria-label={t("Suggested tags")}>
        <input value={filter} placeholder={t("Filter tags…")} aria-label={t("Filter tags…")} onChange={(event) => setFilter(event.target.value)} />
        {shown.map((entry) => {
          const minus = off[key(entry.name)]?.size ?? 0;
          return (
            <button key={entry.name} className={`nav${entry === current ? " on" : ""}`} onClick={() => onTag(entry.name)}>
              <span className="lbl">{entry.name}</span>
              {minus > 0 && <span className="rv-minus">−{minus}</span>}
              <span className="n">{formatNumber(entry.count)}</span>
            </button>
          );
        })}
      </aside>
      <section className="rv-main">
        <div className="rv-bar">
          <b>{current.name}</b>
          <span>{plural(models.length, "suggested for {count} model", "suggested for {count} models")}</span>
          <span className="spacer" />
          <button className="btn ghost small" onClick={() => setSkipped(new Set())}>{t("All on")}</button>
          <button className="btn ghost small" onClick={() => setSkipped(new Set(models.map((item) => item.id)))}>{t("All off")}</button>
        </div>
        <p className="hint">{t("Switch the tag off where it does not fit. ⇧-click switches a range.")}</p>
        <div className="rv-cards">
          {models.map((item, index) => {
            const on = !skipped.has(item.id);
            return (
              <div key={item.id} className={`rv-card${on ? "" : " off"}`}>
                <div className="cover">{item.cover ? <img src={thumbUrl(library, item.cover)} alt="" loading="lazy" /> : <span className="none">{t("No preview")}</span>}</div>
                <div className="name" title={item.path}>{item.title}</div>
                {item.suggested.length > 1 && <div className="also">{t("Also: {tags}", { tags: item.suggested.filter((name) => key(name) !== key(current.name)).join(", ") })}</div>}
                <button className="rv-switch" role="switch" aria-checked={on} onClick={(event) => toggle(index, event.shiftKey)}>
                  <span className="tr" /><b>{on ? t("Keep {tag}", { tag: current.name }) : t("Skip here")}</b>
                </button>
              </div>
            );
          })}
        </div>
        <div className="rv-foot">
          <button className="btn primary" disabled={decide.isPending} onClick={() => resolve(null)}>
            {t("Keep on {kept}, skip {skipped}", { kept: keeping, skipped: models.length - keeping })}
          </button>
          <button className="btn ghost" disabled={decide.isPending} onClick={() => resolve(false)}>{t("Dismiss everywhere")}</button>
        </div>
      </section>
    </div>
  );
}

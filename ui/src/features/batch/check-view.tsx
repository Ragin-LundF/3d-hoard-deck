import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ask } from "@tauri-apps/plugin-dialog";
import { commands, thumbUrl, unwrap, type CheckItemDto_Serialize } from "../../ipc";
import { Icon } from "../../shared/icons";
import { formatNumber, plural, t } from "../../shared/i18n";
import { settingsRoute, type Route } from "../../shared/route";
import { useBatchTags } from "./batch";

type Props = { library: number; tag: string; folder: string; navigate: (route: Route) => void; onMessage: (message: string) => void };
type Filter = "all" | "rule" | "own";

/** A rule hint shows once this many marked models got the tag from a tag rule. */
const RULE_HINT = 3;

/**
 * Check one tag model by model: every model carrying it, grouped by folder, each with Keep / Remove. Nothing changes
 * until "Remove from N"; then it is one batch with Undo.
 */
export function CheckView({ library, tag, folder, navigate, onMessage }: Props) {
  const items = useQuery({ queryKey: ["check", library, tag, folder], queryFn: () => unwrap(commands.checkTag(library, tag, folder)) });
  const [marked, setMarked] = useState<Set<number>>(new Set());
  const [filter, setFilter] = useState<Filter>("all");
  const batch = useBatchTags(library, onMessage);
  const all = items.data ?? [];
  const fromRule = (item: CheckItemDto_Serialize) => item.source !== "own";
  const shown = all.filter((item) => filter === "all" || (filter === "rule" ? fromRule(item) : item.source !== "rule"));
  const groups = useMemo(() => {
    const byFolder = new Map<string, CheckItemDto_Serialize[]>();
    for (const item of shown) byFolder.set(item.folder, [...(byFolder.get(item.folder) ?? []), item]);
    return [...byFolder.entries()].sort((a, b) => b[1].length - a[1].length);
  }, [shown]);
  const toggle = (ids: number[], on: boolean) => {
    const next = new Set(marked);
    for (const id of ids) {
      if (on) next.add(id);
      else next.delete(id);
    }
    setMarked(next);
  };
  const markedRule = all.filter((item) => marked.has(item.id) && fromRule(item)).length;
  const apply = async () => {
    const chosen = all.filter((item) => marked.has(item.id));
    const yes = await ask(
      t("Remove “{tag}” from {count} models? {rule} of them get it from a tag rule; it is hidden there, the rule stays.", { tag, count: chosen.length, rule: markedRule }),
      { title: t("Remove tag"), kind: "warning", okLabel: t("Remove"), cancelLabel: t("Cancel") },
    );
    if (!yes) return;
    batch.mutate({ target: { ids: chosen.map((item) => item.id) }, add: [], remove: [tag] }, { onSuccess: () => setMarked(new Set()) });
  };

  return (
    <div className="review-view">
      <div className="libhead">
        <div className="libtitle">
          <h1>{t("Check “{tag}”", { tag })}</h1>
          <span className="crumbs">{folder || t("Whole library")} · {plural(all.length, "{count} model", "{count} models")}</span>
        </div>
        <div className="filters">
          <div className="seg" role="group" aria-label={t("Where the tag comes from")}>
            {([["all", t("All"), all.length], ["rule", t("Tag rule"), all.filter(fromRule).length], ["own", t("Added by you"), all.filter((item) => item.source !== "rule").length]] as const).map(([id, label, count]) => (
              <button key={id} className={filter === id ? "on" : ""} aria-pressed={filter === id} onClick={() => setFilter(id)}>{label} <span className="n">{formatNumber(count)}</span></button>
            ))}
          </div>
        </div>
      </div>
      <div className="rv-body">
        {items.isPending ? <div className="rv-empty">{t("Loading…")}</div> : all.length === 0 ? <div className="rv-empty">{t("No model carries this tag here.")}</div> : (
          <div className="rv-main">
            <p className="hint">{t("Mark the models where the tag is wrong. Nothing changes until you remove it.")}</p>
            {markedRule >= RULE_HINT && (
              <div className="callout">
                <Icon name="tag" />
                <span className="grow">{t("{count} of the marked models get “{tag}” from a tag rule. Fixing the rule fixes the whole library and future scans.", { count: markedRule, tag })}</span>
                <button className="btn ghost" onClick={() => navigate(settingsRoute("tags"))}>{t("Edit tag rules")}</button>
              </div>
            )}
            {groups.map(([group, models]) => {
              const wrong = models.filter((item) => marked.has(item.id)).length;
              return (
                <section key={group} className="ck-group">
                  <div className="rv-bar">
                    <b>{group || t("Library root")}</b>
                    <span>{plural(models.length, "{count} model", "{count} models")}</span>
                    <span className="spacer" />
                    <button className="btn ghost small" onClick={() => toggle(models.map((item) => item.id), wrong < models.length)}>
                      {wrong < models.length ? t("Mark all {count} wrong", { count: models.length }) : t("Unmark all")}
                    </button>
                  </div>
                  <div className="rv-cards">
                    {models.map((item) => {
                      const remove = marked.has(item.id);
                      return (
                        <div key={item.id} className={`rv-card${remove ? " off" : ""}`}>
                          <div className="cover">{item.cover ? <img src={thumbUrl(library, item.cover)} alt="" loading="lazy" /> : <span className="none">{t("No preview")}</span>}</div>
                          <div className="name" title={item.path}>{item.title}</div>
                          <div className="also"><span className={`src src-${item.source}`}>{item.source === "own" ? t("YOU") : item.source === "rule" ? t("RULE") : t("YOU + RULE")}</span></div>
                          <button className="rv-switch" role="switch" aria-checked={!remove} onClick={() => toggle([item.id], !remove)}>
                            <span className="tr" /><b>{remove ? t("Remove {tag}", { tag }) : t("Keep")}</b>
                          </button>
                        </div>
                      );
                    })}
                  </div>
                </section>
              );
            })}
            <div className="rv-foot">
              <button className="btn primary" disabled={marked.size === 0 || batch.isPending} onClick={() => void apply()}>
                {t("Remove from {count}", { count: marked.size })}
              </button>
              {marked.size > 0 && <button className="btn ghost" onClick={() => setMarked(new Set())}>{t("Clear")}</button>}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

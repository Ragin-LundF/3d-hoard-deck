import { useState } from "react";
import { plural, t } from "../../../shared/i18n";
import type { ReviewRoute, ReviewTab } from "../../../shared/route";
import { AiFolders } from "./ai-folders";
import { ByModel } from "./by-model";
import { ByTag } from "./by-tag";
import { useReviewItems, useUndo } from "./queries";
import type { LogEntry } from "./review-log";
import { Reviewed } from "./reviewed";

type Props = { route: ReviewRoute; libraryName: string; navigate: (route: ReviewRoute) => void; onMessage: (message: string) => void };

/** Review AI tags: decide on suggestions with the previews in view, and choose where the AI works. */
export function ReviewView({ route, libraryName, navigate, onMessage }: Props) {
  const items = useReviewItems(route.library, route.folder);
  const [last, setLast] = useState<LogEntry | null>(null);
  const undo = useUndo(onMessage);
  const go = (patch: Partial<ReviewRoute>) => navigate({ ...route, ...patch });
  const list = items.data ?? [];
  const tabs: [ReviewTab, string][] = [["tag", t("By tag")], ["model", t("By model")], ["reviewed", t("Reviewed")], ["folders", t("AI folders")]];

  return (
    <div className="review-view">
      <div className="libhead">
        <div className="libtitle">
          <h1>{t("Review AI tags")}</h1>
          <span className="crumbs">
            {libraryName} · {route.folder ? route.folder : t("Whole library")}
            {route.folder && <> · <button className="linkish" onClick={() => go({ folder: "" })}>{t("Whole library")}</button></>}
            {" · "}{plural(list.length, "{count} model waiting", "{count} models waiting")}
          </span>
        </div>
        <div className="subtabs" role="tablist">
          {tabs.map(([tab, label]) => (
            <button key={tab} role="tab" aria-selected={route.tab === tab} className={route.tab === tab ? "on" : ""} onClick={() => go({ tab })}>{label}</button>
          ))}
        </div>
      </div>
      <div className="rv-body">
        {route.tab === "tag" && <ByTag library={route.library} items={list} tag={route.tag} onTag={(tag) => navigate({ ...route, tag })} onDone={setLast} onMessage={onMessage} />}
        {route.tab === "model" && <ByModel library={route.library} items={list} onDone={setLast} onMessage={onMessage} />}
        {route.tab === "reviewed" && <Reviewed library={route.library} onMessage={onMessage} />}
        {route.tab === "folders" && <AiFolders library={route.library} onMessage={onMessage} />}
      </div>
      {last && !last.undone && (route.tab === "tag" || route.tab === "model") && (
        <div className="savebar rv-undo" role="status">
          <span className="grow">{last.label}</span>
          <button className="btn ghost" disabled={undo.isPending} onClick={() => undo.mutate(last, { onSuccess: () => setLast(null) })}>{t("Undo")}</button>
          <button className="btn ghost" onClick={() => setLast(null)}>{t("Close")}</button>
        </div>
      )}
    </div>
  );
}

import { t } from "../../../shared/i18n";
import { useUndo } from "./queries";
import { useReviewLog } from "./review-log";

/** What was kept and dropped in this session, each with Undo. */
export function Reviewed({ library, onMessage }: { library: number; onMessage: (message: string) => void }) {
  const log = useReviewLog(library);
  const undo = useUndo(onMessage);
  if (log.length === 0) return <div className="rv-empty">{t("Nothing reviewed yet in this session.")}</div>;
  return (
    <div className="panel">
      {log.map((entry) => (
        <div className="prow2" key={entry.id}>
          <div className="t">
            <b className={entry.undone ? "undone" : ""}>{entry.label}</b>
            <span>{t("{kept} tags kept, {dropped} dropped", { kept: entry.kept, dropped: entry.dropped })}</span>
          </div>
          {entry.undone ? <span className="pill">{t("Undone")}</span> : (
            <button className="btn ghost" disabled={undo.isPending} onClick={() => undo.mutate(entry)}>{t("Undo")}</button>
          )}
        </div>
      ))}
    </div>
  );
}

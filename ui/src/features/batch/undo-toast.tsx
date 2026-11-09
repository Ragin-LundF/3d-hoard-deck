import { t } from "../../shared/i18n";
import { closeLastBatch, useLastBatch, useUndoBatch } from "./batch";

/** "+tag on 12 models · Undo · ✕" after a batch tag change. */
export function UndoToast({ onMessage }: { onMessage: (message: string) => void }) {
  const last = useLastBatch();
  const undo = useUndoBatch(onMessage);
  if (!last) return null;
  return (
    <div className="toast undo-toast" role="status">
      <span>{last.label}</span>
      <button className="linkish" disabled={undo.isPending} onClick={() => undo.mutate(last)}>{t("Undo")}</button>
      <button className="linkish" onClick={closeLastBatch} aria-label={t("Close")}>✕</button>
    </div>
  );
}

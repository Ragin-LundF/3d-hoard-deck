import { commands } from "../../ipc";
import { formatNumber, t } from "../../shared/i18n";
import { useSuggestProgress } from "./queries";

/** In the status bar while an AI suggests tags, with a way to stop. */
export function SuggestStatus() {
  const progress = useSuggestProgress();
  if (!progress?.running) return null;
  return (
    <span className="suggest-status">
      <b>{t("Suggesting tags: {done} of {total}", { done: formatNumber(progress.done), total: formatNumber(progress.total) })}</b>
      {progress.failed > 0 && <span>{t("{n} without an answer", { n: progress.failed })}</span>}
      <button className="linkish" onClick={() => void commands.cancelTagSuggestions()}>{t("Stop")}</button>
    </span>
  );
}

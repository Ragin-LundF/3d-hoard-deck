import { useState } from "react";
import { commands } from "../../ipc";
import { formatNumber, plural, t } from "../../shared/i18n";
import { reviewRoute, useRoute } from "../../shared/route";
import { useSuggestProgress } from "./queries";

/** In the status bar while an AI suggests tags, with a way to stop; afterwards a link to review the result. */
export function SuggestStatus() {
  const progress = useSuggestProgress();
  const [, navigate] = useRoute();
  const [seen, setSeen] = useState<typeof progress>(null);
  if (!progress || progress === seen) return null;
  if (!progress.running) {
    const answered = progress.done - progress.failed;
    if (progress.total === 0) return null;
    return (
      <span className="suggest-status">
        <b>{plural(answered, "{count} model got suggestions", "{count} models got suggestions")}</b>
        {progress.failed > 0 && <span>{t("{n} without an answer", { n: progress.failed })}</span>}
        <button className="linkish" onClick={() => { setSeen(progress); navigate(reviewRoute(progress.library)); }}>{t("Review ›")}</button>
        <button className="linkish" onClick={() => setSeen(progress)} aria-label={t("Close")}>✕</button>
      </span>
    );
  }
  return (
    <span className="suggest-status">
      <b>{t("Suggesting tags: {done} of {total}", { done: formatNumber(progress.done), total: formatNumber(progress.total) })}</b>
      {progress.failed > 0 && <span>{t("{n} without an answer", { n: progress.failed })}</span>}
      <button className="linkish" onClick={() => void commands.cancelTagSuggestions()}>{t("Stop")}</button>
    </span>
  );
}

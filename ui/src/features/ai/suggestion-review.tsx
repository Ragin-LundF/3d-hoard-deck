import { useQuery } from "@tanstack/react-query";
import { commands, unwrap } from "../../ipc";
import { formatNumber, plural, t } from "../../shared/i18n";
import { reviewRoute, useRoute } from "../../shared/route";
import { useAiEnabled } from "./queries";

/** Chips shown above the tag list; the rest waits in the review screen. */
const SHOWN = 12;

/**
 * One compact row above the tag browser: the most suggested tags, each opening the review on that tag, and a link to
 * review them all. Kept small so the tag list below keeps its room however many tags the AI suggested.
 */
export function SuggestionReview({ library }: { library: number }) {
  const enabled = useAiEnabled();
  const [, navigate] = useRoute();
  const counts = useQuery({ queryKey: ["suggestedTags", library], queryFn: () => unwrap(commands.suggestedTagCounts(library)), enabled });
  const list = counts.data ?? [];
  if (!enabled || list.length === 0) return null;
  return (
    <section className="review" aria-label={t("Suggested by AI")}>
      <b>{t("Suggested by AI")}</b>
      <div className="review-list">
        {list.slice(0, SHOWN).map(({ name, count }) => (
          <button key={name} className="chip auto ai" onClick={() => navigate(reviewRoute(library, "tag", name))} title={t("See the models it was suggested for")}>
            {name} <span className="n">{formatNumber(count)}</span>
          </button>
        ))}
      </div>
      <button className="btn small" onClick={() => navigate(reviewRoute(library))}>
        {plural(list.length, "Review {count} tag ›", "Review {count} tags ›")}
      </button>
    </section>
  );
}

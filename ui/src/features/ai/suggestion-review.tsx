import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap } from "../../ipc";
import { formatNumber, plural, t } from "../../shared/i18n";
import { useAiEnabled } from "./queries";

type Props = { library: number; onMessage: (message: string) => void };

/** All tags an AI suggested in a library: keep one for every model it was suggested for, or dismiss it everywhere. */
export function SuggestionReview({ library, onMessage }: Props) {
  const enabled = useAiEnabled();
  const queryClient = useQueryClient();
  const counts = useQuery({ queryKey: ["suggestedTags", library], queryFn: () => unwrap(commands.suggestedTagCounts(library)), enabled });
  const resolve = useMutation({
    mutationFn: ({ tag, keep }: { tag: string; keep: boolean }) => unwrap(commands.resolveSuggestedTag(library, tag, keep)),
    onSuccess: (changed, { tag, keep }) => {
      onMessage(keep ? plural(changed, "{tag} kept on {count} model", "{tag} kept on {count} models").replace("{tag}", tag) : t("{tag} dismissed", { tag }));
      void queryClient.invalidateQueries();
    },
    onError: (error) => onMessage(describeError(error)),
  });
  const list = counts.data ?? [];
  if (!enabled || list.length === 0) return null;
  return (
    <section className="review" aria-label={t("Suggested by AI")}>
      <div className="review-head">
        <b>{t("Suggested by AI")}</b>
        <span>{t("Keep a tag for every model it was suggested for, or dismiss it. Nothing is changed before that.")}</span>
      </div>
      <div className="review-list">
        {list.map(({ name, count }) => (
          <span className="chip auto ai" key={name}>
            {name} <span className="n">{formatNumber(count)}</span>
            <button className="btn ghost small" disabled={resolve.isPending} onClick={() => resolve.mutate({ tag: name, keep: true })}>{t("Keep for all")}</button>
            <button className="btn ghost small" disabled={resolve.isPending} onClick={() => resolve.mutate({ tag: name, keep: false })}>{t("Dismiss")}</button>
          </span>
        ))}
      </div>
    </section>
  );
}

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap, type ModelDetailDto_Serialize } from "../../ipc";
import { Icon } from "../../shared/icons";
import { t } from "../../shared/i18n";
import { useAiEnabled } from "./queries";

type Props = { model: ModelDetailDto_Serialize; onMessage: (message: string) => void };

/** Tags an AI suggested for this model, with keep and dismiss, and a button to ask. Nothing renders while AI is off. */
export function AiTags({ model, onMessage }: Props) {
  const enabled = useAiEnabled();
  const id = model.summary.id;
  const queryClient = useQueryClient();
  const suggestions = useQuery({ queryKey: ["suggestions", id], queryFn: () => unwrap(commands.tagSuggestions(id)), enabled });
  const ask = useMutation({
    mutationFn: () => unwrap(commands.suggestTags(id)),
    onSuccess: (tags) => {
      queryClient.setQueryData(["suggestions", id], tags);
      if (tags.length === 0) onMessage(t("The AI had no new tags for this model."));
    },
    onError: (error) => onMessage(describeError(error)),
  });
  const resolve = useMutation({
    mutationFn: ({ tag, keep }: { tag: string; keep: boolean }) => unwrap(commands.resolveTagSuggestion(id, tag, keep)),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["suggestions", id] });
      void queryClient.invalidateQueries({ queryKey: ["model", id] });
      void queryClient.invalidateQueries({ queryKey: ["suggestedTags"] });
    },
    onError: (error) => onMessage(describeError(error)),
  });
  if (!enabled) return null;
  const tags = suggestions.data ?? [];
  return (
    <div className="ruletags">
      {tags.length > 0 && <span className="rt-label">{t("Suggested by AI")}</span>}
      {tags.map((tag) => (
        <span className="chip auto ai" key={tag}>
          {tag}
          <button className="rt-act" disabled={resolve.isPending} onClick={() => resolve.mutate({ tag, keep: true })} aria-label={t("Keep {tag} as a tag", { tag })} title={t("Keep as a tag")}>
            <Icon name="check" />
          </button>
          <button className="rt-act" disabled={resolve.isPending} onClick={() => resolve.mutate({ tag, keep: false })} aria-label={t("Dismiss {tag}", { tag })} title={t("Dismiss")}>
            <Icon name="close" />
          </button>
        </span>
      ))}
      <button className="btn ghost small" onClick={() => ask.mutate()} disabled={ask.isPending}>
        <Icon name="tag" />{ask.isPending ? t("Asking the AI…") : tags.length > 0 ? t("Ask again") : t("Suggest tags")}
      </button>
    </div>
  );
}

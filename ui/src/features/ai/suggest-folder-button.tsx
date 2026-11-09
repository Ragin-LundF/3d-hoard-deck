import { useQuery } from "@tanstack/react-query";
import { commands, describeError, unwrap } from "../../ipc";
import { reviewRoute, useRoute } from "../../shared/route";
import { Icon } from "../../shared/icons";
import { plural, t } from "../../shared/i18n";
import { useAiEnabled, useAiSettings, useSuggestProgress } from "./queries";
import { startSuggestions } from "./start-run";

type Props = { library: number; folder: string; onMessage: (message: string) => void };

/** Starts an AI run over a folder and its subfolders; only shown when AI is set up. */
export function SuggestFolderButton({ library, folder, onMessage }: Props) {
  const enabled = useAiEnabled();
  const remote = useAiSettings().data?.remote ?? true;
  const running = useSuggestProgress()?.running ?? false;
  const [, navigate] = useRoute();
  const pending = useQuery({ queryKey: ["pending", library, folder], queryFn: () => unwrap(commands.pendingSuggestions(library, folder)), enabled });
  if (!enabled) return null;
  const start = () =>
    startSuggestions(library, folder, remote)
      .then((total) => total !== null && onMessage(total === 0 ? t("Every model here has suggestions already.") : plural(total, "Suggesting tags for {count} model…", "Suggesting tags for {count} models…")))
      .catch((error) => onMessage(describeError(error)));
  return (
    <>
    {(pending.data ?? 0) > 0 && (
      <button className="btn ghost" onClick={() => navigate(reviewRoute(library, "tag", "", folder))} title={t("Review the AI's suggestions for this folder")}>
        {t("Review {n}", { n: pending.data ?? 0 })}
      </button>
    )}
    <button className="btn ghost" disabled={running} onClick={() => void start()} title={t("Asks the AI about every model in this folder that has no suggestions yet. Nothing is changed until you keep a tag.")}>
      <Icon name="tag" />{t("Suggest tags")}
    </button>
    </>
  );
}

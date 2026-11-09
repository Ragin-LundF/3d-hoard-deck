import { commands, describeError, unwrap } from "../../ipc";
import { Icon } from "../../shared/icons";
import { plural, t } from "../../shared/i18n";
import { useAiEnabled, useSuggestProgress } from "./queries";

type Props = { library: number; folder: string; onMessage: (message: string) => void };

/** Starts an AI run over a folder and its subfolders; only shown when AI is set up. */
export function SuggestFolderButton({ library, folder, onMessage }: Props) {
  const enabled = useAiEnabled();
  const running = useSuggestProgress()?.running ?? false;
  if (!enabled) return null;
  const start = () =>
    unwrap(commands.startTagSuggestions(library, folder))
      .then((total) => onMessage(total === 0 ? t("Every model here has suggestions already.") : plural(total, "Suggesting tags for {count} model…", "Suggesting tags for {count} models…")))
      .catch((error) => onMessage(describeError(error)));
  return (
    <button className="btn ghost" disabled={running} onClick={() => void start()} title={t("Asks the AI about every model in this folder that has no suggestions yet. Nothing is changed until you keep a tag.")}>
      <Icon name="tag" />{t("Suggest tags")}
    </button>
  );
}

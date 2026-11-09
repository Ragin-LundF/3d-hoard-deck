import { ask } from "@tauri-apps/plugin-dialog";
import { commands, unwrap } from "../../ipc";
import { plural, t } from "../../shared/i18n";

/**
 * Starts an AI run over `folder` ("" = whole library). With a cloud service each picture costs money, so the number of
 * models is confirmed first. Returns how many models are asked about, or null when the user said no.
 */
export async function startSuggestions(library: number, folder: string, remote: boolean): Promise<number | null> {
  if (remote) {
    const count = await unwrap(commands.countTagSuggestions(library, folder));
    if (count === 0) return 0;
    const yes = await ask(
      plural(count, "This sends {count} preview picture to the AI service. Cloud services charge for each one.", "This sends {count} preview pictures to the AI service. Cloud services charge for each one."),
      { title: t("Suggest tags"), kind: "warning", okLabel: t("Send"), cancelLabel: t("Cancel") },
    );
    if (!yes) return null;
  }
  return unwrap(commands.startTagSuggestions(library, folder));
}

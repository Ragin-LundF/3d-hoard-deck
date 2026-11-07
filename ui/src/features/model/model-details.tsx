import type { LibraryDto, ModelDetailDto_Serialize } from "../../ipc";
import { formatBytes } from "../../shared/format";
import { TagsInput } from "../organize/tags-input";
import { useSaveMeta } from "../organize/use-save-meta";
import { t } from "../../shared/i18n";

type Props = { model: ModelDetailDto_Serialize; library: LibraryDto | undefined; onMessage: (message: string) => void };

/** Tags, notes and facts about the model. Tags and notes are saved as soon as they change. */
export function ModelDetails({ model, library, onMessage }: Props) {
  const { summary, meta } = model;
  const save = useSaveMeta(model, onMessage);
  return (
    <div className="details">
      <div className="fgroup">
        <h4>{t("Tags")}</h4>
        <TagsInput library={summary.library} tags={meta.tags} onChange={(tags) => save({ tags })} />
      </div>
      <div className="fgroup">
        <h4>{t("Notes")}</h4>
        <textarea
          className="notes"
          key={summary.id}
          defaultValue={meta.notes}
          placeholder={t("Print settings, painting ideas, what to watch out for…")}
          aria-label={t("Notes")}
          onBlur={(event) => event.target.value.trim() !== meta.notes && save({ notes: event.target.value })}
        />
      </div>
      <dl className="kv">
        <dt>{t("Formats")}</dt><dd>{summary.formats.join(", ").toUpperCase() || "–"}</dd>
        <dt>{t("Files")}</dt><dd>{summary.fileCount}</dd>
        <dt>{t("Size")}</dt><dd>{formatBytes(summary.totalSize)}</dd>
        <dt>{t("Released")}</dt><dd>{summary.released ?? "–"}</dd>
        <dt>{t("Collection")}</dt><dd>{summary.collection ?? "–"}</dd>
        <dt>{t("Library")}</dt><dd>{library?.name ?? "–"}</dd>
        <dt>{t("Cover")}</dt>
        <dd>
          {meta.cover ? (
            <>{t("Picked by you")} · <button className="linkish" onClick={() => save({ cover: null })}>{t("Use the detected cover")}</button></>
          ) : (
            t("Detected automatically. Show an image and choose “Use as cover” to change it.")
          )}
        </dd>
      </dl>
    </div>
  );
}

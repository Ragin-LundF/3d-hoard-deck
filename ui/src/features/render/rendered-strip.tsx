import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { commands, describeError, unwrap, type LibraryDto } from "../../ipc";
import { plural, t } from "../../shared/i18n";
import { showUndo } from "../batch/batch";
import { renderedUrl } from "./rendered-cover";

type Props = { library: LibraryDto; ids: number[]; onMessage: (message: string) => void };

/** Above the grid while rendered previews wait here: a glance at them and "Save all" (or "Keep all" read-only). */
export function RenderedStrip({ library, ids, onMessage }: Props) {
  const queryClient = useQueryClient();
  const [busy, setBusy] = useState(false);
  if (ids.length === 0) return null;
  const canWrite = library.access !== "readOnly";
  const keepAll = async () => {
    setBusy(true);
    try {
      const kept = await unwrap(commands.keepRenderedPreviews(ids));
      if (kept.kept > 0) showUndo(plural(kept.kept, "{count} preview kept", "{count} previews kept"), () => unwrap(commands.resetRenderedPreviews(ids, "cached")));
      else onMessage(plural(kept.written, "{count} preview saved next to its file.", "{count} previews saved next to their files."));
      await queryClient.invalidateQueries();
    } catch (error) {
      onMessage(describeError(error));
    }
    setBusy(false);
  };
  return (
    <div className="rendered-strip">
      <div className="thumbs">{ids.slice(0, 6).map((id) => <img key={id} src={renderedUrl(id)} alt="" />)}</div>
      <span className="grow">
        {plural(ids.length, "{count} rendered preview for a model without a picture", "{count} rendered previews for models without a picture")}
        <span className="hint">{canWrite ? t("saved as preview_<file>.png in each model folder") : t("kept in the app; nothing is written into this library")}</span>
      </span>
      <button className="btn primary small" disabled={busy} onClick={() => void keepAll()}>
        {canWrite ? t("Save all {count}", { count: ids.length }) : t("Keep all {count}", { count: ids.length })}
      </button>
    </div>
  );
}

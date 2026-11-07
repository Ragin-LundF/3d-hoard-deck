import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { commands, describeError, unwrap } from "../../ipc";
import { folderName } from "../../shared/format";
import { t } from "../../shared/i18n";

type Mode = "rename" | "move";
type Props = { model: number; path: string; onDone: (message: string) => void };

/** Rename on disk or move to another folder; only shown for libraries in "Metadata files, rename and move" mode. */
export function MoveOnDisk({ model, path, onDone }: Props) {
  const [mode, setMode] = useState<Mode | null>(null);
  const queryClient = useQueryClient();
  const move = useMutation({
    mutationFn: ({ mode, value }: { mode: Mode; value: string }) =>
      unwrap(commands.moveModel(model, mode === "rename" ? value : null, mode === "move" ? value : null)),
    onSuccess: async (change) => {
      await queryClient.invalidateQueries();
      onDone(change.warning ?? t("Done. Undo it under Settings › Recent file changes."));
    },
  });
  const submit = (value: string) => {
    const trimmed = value.trim();
    if (mode && trimmed) move.mutate({ mode, value: trimmed });
  };

  if (!mode) {
    return (
      <>
        <button className="btn ghost" onClick={() => setMode("rename")}>{t("Rename on disk…")}</button>
        <button className="btn ghost" onClick={() => setMode("move")}>{t("Move to folder…")}</button>
      </>
    );
  }
  const label = mode === "rename" ? t("New name on disk") : t("Target folder, relative to the library");
  return (
    <form className="disk-form" onSubmit={(event) => { event.preventDefault(); submit(new FormData(event.currentTarget).get("value")?.toString() ?? ""); }}>
      <input
        name="value"
        autoFocus
        aria-label={label}
        placeholder={label}
        defaultValue={mode === "rename" ? folderName(path) : ""}
        onKeyDown={(event) => { if (event.key === "Escape") setMode(null); }}
      />
      <button className="btn primary" type="submit" disabled={move.isPending}>{mode === "rename" ? t("Rename") : t("Move")}</button>
      <button className="btn ghost" type="button" onClick={() => setMode(null)}>{t("Cancel")}</button>
      {move.isError && <span className="err">{describeError(move.error)}</span>}
    </form>
  );
}

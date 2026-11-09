import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap, type LibraryDto } from "../../ipc";
import { t, formatDate } from "../../shared/i18n";

const STATUS: Record<string, string> = {
  undone: t("Undone"),
  failed: t("Did not work, nothing changed"),
  pending: t("Interrupted, checked at next start"),
  undoing: t("Interrupted, checked at next start"),
};

type Props = { libraries: LibraryDto[]; onError: (message: string) => void };

/** The latest renames and moves on disk, each with Undo. */
export function FileJournal({ libraries, onError }: Props) {
  const queryClient = useQueryClient();
  const journal = useQuery({ queryKey: ["file-journal"], queryFn: () => unwrap(commands.fileJournal(20)) });
  const undo = useMutation({
    mutationFn: (id: number) => unwrap(commands.undoFileOperation(id)),
    onSuccess: async (change) => {
      await queryClient.invalidateQueries();
      if (change.warning) onError(change.warning);
    },
    onError: (error) => onError(describeError(error)),
  });
  const entries = journal.data ?? [];
  if (entries.length === 0) return <p className="lead">{t("No files were renamed or moved yet.")}</p>;
  const libraryName = (id: number) => libraries.find((library) => library.id === id)?.name ?? t("Removed library");
  return (
    <div className="panel">
      {entries.map((entry) => (
        <div className="prow2" key={entry.id}>
          <div className="t">
            <b>{entry.extract ? t("Extracted {archive} → {folder}", { archive: entry.from, folder: entry.to }) : `${entry.from} → ${entry.to}`}</b>
            <span>{libraryName(entry.library)} · {formatDate(new Date((entry.doneAt ?? 0) * 1000))}</span>
          </div>
          {entry.extract
            ? <span className={`pill${entry.status === "failed" ? " err" : ""}`}>{entry.status === "done" ? t("Extracted") : entry.status === "undone" ? t("Discarded") : STATUS[entry.status] ?? entry.status}</span>
            : entry.status === "done"
            ? <button className="btn" disabled={undo.isPending} onClick={() => undo.mutate(entry.id)}>{t("Undo")}</button>
            : <span className={`pill${entry.status === "failed" ? " err" : ""}`}>{STATUS[entry.status] ?? entry.status}</span>}
        </div>
      ))}
    </div>
  );
}

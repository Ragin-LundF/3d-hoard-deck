import { useMutation } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { describeError, commands, events, IpcFailure, unwrap, type LibraryDto } from "../../ipc";
import { formatBytes } from "../../shared/format";
import { Icon } from "../../shared/icons";
import { menuProps } from "../../shared/context-menu";
import { copyPath, revealPath } from "../../shared/file-actions";
import { t, plural } from "../../shared/i18n";

type Props = { libraries: LibraryDto[]; onOpenModel: (id: number) => void; onMessage: (message: string) => void };

/** Finds identical model files across all libraries. Only reads; nothing is deleted from here. */
export function DuplicatesView({ libraries, onOpenModel, onMessage }: Props) {
  const find = useMutation({ mutationFn: () => unwrap(commands.findDuplicates()) });
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  useEffect(() => {
    const unlisten = events.duplicatesProgressEvent.listen(({ payload }) => setProgress(payload));
    return () => void unlisten.then((stop) => stop());
  }, []);
  const comparing = find.isPending && progress && progress.total > 0
    ? t("Comparing files… {percent} %", { percent: Math.round((progress.done / progress.total) * 100) })
    : t("Comparing files…");
  const groups = find.data ?? [];
  const wasted = groups.reduce((sum, group) => sum + (group.size ?? 0) * (group.files.length - 1), 0);
  const libraryName = (id: number) => libraries.find((library) => library.id === id)?.name ?? "";
  return (
    <div className="setpane">
      <h1>{t("Duplicates")}</h1>
      <p className="lead">{t("Model files with exactly the same content, in any library. Files are compared by size, then by content; this reads candidate files and can take a while on a network share. Nothing is changed.")}</p>
      <div>
        <button className="btn primary" onClick={() => { setProgress(null); find.mutate(); }} disabled={find.isPending}>
          <Icon name="search" />{find.isPending ? comparing : find.data ? t("Search again") : t("Find duplicates")}
        </button>
        {find.isPending && <button className="btn ghost" onClick={() => void commands.cancelDuplicates()}>{t("Cancel")}</button>}
      </div>
      {find.isError && (find.error instanceof IpcFailure && find.error.code === "cancelled"
        ? <p className="lead">{t("Search cancelled.")}</p>
        : <div className="err">{describeError(find.error)}</div>)}
      {find.data && (
        <p className="lead">
          {groups.length === 0 ? t("No duplicates found.") : `${plural(groups.length, "{count} group", "{count} groups")} · ${t("{size} could be freed", { size: formatBytes(wasted) })}`}
        </p>
      )}
      {groups.map((group, index) => (
        <div className="panel" key={index}>
          <div className="prow2"><div className="t"><b>{plural(group.files.length, "{count} copy", "{count} copies")} · {t("{size} each", { size: formatBytes(group.size) })}</b></div></div>
          {group.files.map((file) => (
            <div className="prow2" key={`${file.library}/${file.path}`} {...menuProps(() => {
              const root = libraries.find((library) => library.id === file.library)?.root ?? null;
              return [
                { label: t("Open model"), hidden: file.model === null, onSelect: () => onOpenModel(file.model ?? 0) },
                { label: t("Show in folder"), hidden: root === null, onSelect: () => root !== null && revealPath(root, file.path, onMessage) },
                { label: t("Copy path"), hidden: root === null, onSelect: () => root !== null && copyPath(root, file.path, onMessage) },
              ];
            })}>
              <div className="t">
                <span className="mono">{file.path}</span>
                <span>{libraryName(file.library)}</span>
              </div>
              {file.model !== null && <button className="btn ghost" onClick={() => onOpenModel(file.model ?? 0)}>{t("Open model")}</button>}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

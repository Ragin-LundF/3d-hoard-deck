import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { commands, describeError, events, unwrap, type LibraryDto } from "../../ipc";
import { formatDate, plural, t } from "../../shared/i18n";

type RestoreReport = Awaited<ReturnType<typeof restore>>;
const restore = (library: number, filesWin: boolean, backup: string | null) => unwrap(commands.restoreFromSidecars(library, filesWin, backup));

/** What a restore did, in one sentence or two. */
export function restoreMessage(report: RestoreReport): string {
  const parts = [
    report.restored === 0 && report.rulesRestored === 0
      ? t("Nothing to restore: the index already has everything the files hold.")
      : plural(report.restored, "Restored {count} model or folder.", "Restored {count} models and folders."),
  ];
  if (report.rulesRestored > 0) parts.push(plural(report.rulesRestored, "{count} scan or folder rule came back.", "{count} scan and folder rules came back."));
  if (report.tagRulesRestored > 0) parts.push(plural(report.tagRulesRestored, "{count} tag rule came back.", "{count} tag rules came back."));
  if (report.droppedMembers > 0) parts.push(plural(report.droppedMembers, "{count} collection member no longer exists.", "{count} collection members no longer exist."));
  if (report.unreadable.length > 0) parts.push(plural(report.unreadable.length, "{count} file could not be read.", "{count} files could not be read."));
  return parts.join(" ");
}

/** When saved tag rules differ from the app's, asks before taking them over. */
export async function offerTagRules(library: number, report: RestoreReport, backup: string | null, onMessage: (message: string) => void) {
  if (report.tagRulesOffered === 0) return;
  const take = await ask(
    plural(report.tagRulesOffered, "This library has {count} tag rule saved. Use it instead of your current tag rules?", "This library has {count} tag rules saved. Use them instead of your current tag rules?"),
    { title: t("Saved tag rules"), kind: "info", okLabel: t("Use them"), cancelLabel: t("Keep mine") },
  );
  if (!take) return;
  const taken = await unwrap(commands.useSavedTagRules(library, backup));
  onMessage(plural(taken, "Took over {count} tag rule.", "Took over {count} tag rules."));
}

type Props = { library: LibraryDto; onMessage: (message: string) => void };

/**
 * Settings › Library › Import & rebuild: write every metadata file now, a backup file for read-only libraries, and the
 * restore from files or a backup.
 */
export function MetadataFiles({ library, onMessage }: Props) {
  const queryClient = useQueryClient();
  const writes = library.metadata === "indexAndSidecar" && library.access !== "readOnly";
  const status = useQuery({ queryKey: ["metadata-status", library.id], queryFn: () => unwrap(commands.metadataStatus(library.id)) });
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [filesWin, setFilesWin] = useState(false);
  const [failed, setFailed] = useState<{ file: string; reason: string }[]>([]);
  useEffect(() => {
    const unlisten = events.metadataProgressEvent.listen(({ payload }) => {
      if (payload.library === library.id) setProgress({ done: payload.done, total: payload.total });
    });
    return () => void unlisten.then((stop) => stop());
  }, [library.id]);

  const write = useMutation({
    mutationFn: () => unwrap(commands.writeMetadataFiles(library.id)),
    onSuccess: (report) => {
      setFailed(report.failed);
      onMessage(t("Wrote {written}, unchanged {unchanged}, failed {failed}.", { written: report.written, unchanged: report.unchanged, failed: report.failed.length }));
    },
    onError: (error) => onMessage(describeError(error)),
    onSettled: () => {
      setProgress(null);
      void queryClient.invalidateQueries({ queryKey: ["metadata-status", library.id] });
    },
  });
  const retry = useMutation({
    mutationFn: () => unwrap(commands.retryMetadataFiles(library.id)),
    onSuccess: (next) => onMessage(next.pending === 0 ? t("All metadata files are written.") : plural(next.pending, "{count} file still could not be written.", "{count} files still could not be written.")),
    onError: (error) => onMessage(describeError(error)),
    onSettled: () => void queryClient.invalidateQueries({ queryKey: ["metadata-status", library.id] }),
  });
  const backup = useMutation({
    mutationFn: async () => {
      const path = await save({ title: t("Export backup file"), defaultPath: `${library.name}.3dhoarddeck-backup.json`, filters: [{ name: t("Backup file"), extensions: ["json"] }] });
      return path ? unwrap(commands.exportBackup(library.id, path)) : null;
    },
    onSuccess: (entries) => entries !== null && onMessage(plural(entries, "Backup written with {count} model or folder.", "Backup written with {count} models and folders.")),
    onError: (error) => onMessage(describeError(error)),
  });
  const restoreFrom = useMutation({
    mutationFn: async (fromBackup: boolean) => {
      let path: string | null = null;
      if (fromBackup) {
        const chosen = await open({ title: t("Restore from backup file"), filters: [{ name: t("Backup file"), extensions: ["json"] }] });
        if (typeof chosen !== "string") return null;
        path = chosen;
      }
      const report = await restore(library.id, filesWin, path);
      return { report, path };
    },
    onSuccess: async (result) => {
      if (!result) return;
      await queryClient.invalidateQueries();
      onMessage(restoreMessage(result.report));
      await offerTagRules(library.id, result.report, result.path, onMessage).catch((error) => onMessage(describeError(error)));
    },
    onError: (error) => onMessage(describeError(error)),
  });
  const pending = status.data?.pending ?? 0;
  const exportedAt = status.data?.exportedAt;

  return (
    <>
      <div className="prow2">
        <div className="t">
          <b>{t("Write metadata files now")}</b>
          <span>
            {writes
              ? t("Writes a .3dhoarddeck file for every model and folder with names, tags, notes, marks, covers or folder decisions, plus the library's rules, tag groups and collections at its root. Unchanged files are not touched.")
              : t("This library keeps no metadata files. Allow them under Access & storage, or export a backup file below.")}
          </span>
          {writes && (
            <span className="muted">
              {exportedAt ? t("Last written in full: {date}", { date: formatDate(new Date(exportedAt * 1000)) }) : t("Not written in full yet.")}
              {pending > 0 && <> · <span className="err">{plural(pending, "{count} file not written yet", "{count} files not written yet")}</span>{" "}
                <button className="linkish" disabled={retry.isPending} onClick={() => retry.mutate()}>{t("Try again")}</button></>}
            </span>
          )}
          {progress && progress.total > 0 && (
            <div className="xprogress" role="progressbar" aria-valuemin={0} aria-valuemax={progress.total} aria-valuenow={progress.done}>
              <span style={{ width: `${(progress.done / progress.total) * 100}%` }} />
            </div>
          )}
          {failed.length > 0 && (
            <details className="muted">
              <summary>{plural(failed.length, "{count} file could not be written", "{count} files could not be written")}</summary>
              <ul>{failed.slice(0, 50).map((item) => <li key={item.file}>{item.file}: {item.reason}</li>)}</ul>
            </details>
          )}
        </div>
        {write.isPending
          ? <button className="btn" onClick={() => void unwrap(commands.cancelMetadataFiles())}>{t("Cancel")}</button>
          : <button className="btn" disabled={!writes} onClick={() => write.mutate()}>{t("Write")}</button>}
      </div>
      <div className="prow2">
        <div className="t">
          <b>{t("Export backup file")}</b>
          <span>{t("One file with everything you decided for this library, saved where you choose (another drive, for example). Works for read-only libraries too; nothing is written into the library.")}</span>
        </div>
        <button className="btn" disabled={backup.isPending} onClick={() => backup.mutate()}>{t("Export…")}</button>
      </div>
      <div className="prow2">
        <div className="t">
          <b>{t("Restore from metadata files")}</b>
          <span>{t("Brings back what the .3dhoarddeck files in this library hold: names, tags, notes, marks, covers, dates added, folder rules, tag groups and collections. Useful after a lost index or on another computer. Only reads the library.")}</span>
          <div className="seg mini" role="group" aria-label={t("When the index and a file differ")}>
            <button className={filesWin ? "" : "on"} aria-pressed={!filesWin} onClick={() => setFilesWin(false)}>{t("Newer wins")}</button>
            <button className={filesWin ? "on" : ""} aria-pressed={filesWin} onClick={() => setFilesWin(true)}>{t("Files win")}</button>
          </div>
        </div>
        <div className="row-actions">
          <button className="btn" disabled={restoreFrom.isPending} onClick={() => restoreFrom.mutate(false)}>{t("Restore")}</button>
          <button className="btn ghost" disabled={restoreFrom.isPending} onClick={() => restoreFrom.mutate(true)}>{t("From backup file…")}</button>
        </div>
      </div>
    </>
  );
}

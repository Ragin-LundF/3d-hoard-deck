import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { commands, describeError, events, thumbUrl, unwrap, type LibraryDto } from "../../ipc";
import { fileName, folderName, formatBytes } from "../../shared/format";
import { plural, t } from "../../shared/i18n";

/** An archive and the model it belongs to (its own, or the folder's model when archives lie alone in a folder). */
export type ArchiveItem = { model: number; archive: string };
export type PendingExtraction = { journal: number; archive: string; folder: string };

type Plan = Awaited<ReturnType<typeof planFor>>[number] & { model: number };
type Extracted = Awaited<ReturnType<typeof extractOne>>;
type Result = { plan: Plan; name: string; done?: Extracted; error?: string };
type Step = "plan" | "run" | "review" | "confirm" | "noTrash" | "noTrashDiscard" | "done";

const planFor = (item: ArchiveItem, password: string | null) => unwrap(commands.planExtraction(item.model, [item.archive], password));
const extractOne = (plan: Plan, name: string, password: string | null) =>
  unwrap(commands.extractArchive(plan.model, plan.archive, name, password));

type Props = {
  library: LibraryDto;
  items: ArchiveItem[];
  /** Already extracted archives: the dialog starts at deleting them. */
  pending?: PendingExtraction[];
  onClose: () => void;
  onMessage: (message: string) => void;
  onOpenFolder: (library: number, folder: string) => void;
};

/**
 * Extract archives into new folders, check the result, then delete the archives (or keep both, or discard the
 * folders). Files appear only when everything is unpacked and verified; nothing is deleted without the user's yes.
 */
export function ExtractDialog({ library, items, pending, onClose, onMessage, onOpenFolder }: Props) {
  const queryClient = useQueryClient();
  const [password, setPassword] = useState<string | null>(null);
  const [typed, setTyped] = useState("");
  const [names, setNames] = useState<Record<string, string>>({});
  const [step, setStep] = useState<Step>(pending ? "confirm" : "plan");
  const [results, setResults] = useState<Result[]>([]);
  const [current, setCurrent] = useState<string | null>(null);
  const [progress, setProgress] = useState<{ done: number; total: number; file: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const [skipped, setSkipped] = useState<Set<string>>(new Set());
  const [minimized, setMinimized] = useState(false);
  const cancelled = useRef(false);
  /** Journals already settled, so a retry after "no trash" does not touch them again. */
  const settled = useRef(new Set<number>());

  const plans = useQuery({
    queryKey: ["extract-plan", items, password],
    queryFn: async () => (await Promise.all(items.map(async (item) => (await planFor(item, password)).map((plan) => ({ ...plan, model: item.model }))))).flat(),
  });
  // Started from "Extracted to … · Delete archive": the plans give the parts to delete and the kind of drive.
  useEffect(() => {
    if (!pending || !plans.data) return;
    setResults(pending.flatMap((entry) => {
      const plan = plans.data.find((candidate) => candidate.archive === entry.archive);
      return plan ? [{ plan, name: folderName(entry.folder), done: { journal: entry.journal, folder: entry.folder, files: 0, bytes: 0, renamed: [], skipped: [], unwrapped: null, pictures: [], warning: null } }] : [];
    }));
  }, [pending, plans.data]);
  useEffect(() => {
    const unlisten = events.extractProgressEvent.listen(({ payload }) => setProgress({ done: payload.done ?? 0, total: payload.total ?? 0, file: payload.file }));
    return () => void unlisten.then((stop) => stop());
  }, []);

  const ready = (plans.data ?? []).filter((plan) => !plan.problem && !skipped.has(plan.archive));
  const toggle = (archive: string) => {
    const next = new Set(skipped);
    if (next.has(archive)) next.delete(archive);
    else next.add(archive);
    setSkipped(next);
  };
  const needsPassword = (plans.data ?? []).some((plan) => plan.encrypted && plan.files === 0) || (plans.data ?? []).some((plan) => plan.problem === "wrong password");
  const nameOf = (plan: Plan) => names[plan.archive] ?? plan.folder;
  const extracted = results.filter((result) => result.done);
  const failed = results.filter((result) => result.error);
  const network = extracted.some((result) => result.plan.network);
  const partFiles = extracted.flatMap((result) => result.plan.parts);

  const run = async () => {
    setStep("run");
    cancelled.current = false;
    const done: Result[] = [];
    for (const plan of ready) {
      if (cancelled.current) break;
      setCurrent(plan.archive);
      setProgress(null);
      try {
        done.push({ plan, name: nameOf(plan), done: await extractOne(plan, nameOf(plan), password) });
      } catch (error) {
        done.push({ plan, name: nameOf(plan), error: describeError(error) });
      }
      setResults([...done]);
    }
    setCurrent(null);
    await queryClient.invalidateQueries();
    setMinimized(false);
    setStep("review");
  };
  const cancel = () => {
    cancelled.current = true;
    void unwrap(commands.cancelExtraction());
  };
  /** Settles every extracted archive the same way; `no_trash` sends the user to the second question. */
  const finish = async (choice: "delete" | "deletePermanently" | "discard" | "discardPermanently") => {
    setBusy(true);
    setFailure(null);
    let trashed = 0;
    let removed = 0;
    const warnings: string[] = [];
    const discarding = choice.startsWith("discard");
    try {
      for (const result of extracted) {
        if (!result.done || settled.current.has(result.done.journal)) continue;
        const finished = await unwrap(commands.finishExtraction(result.done.journal, choice === "delete" && result.plan.network ? "deletePermanently" : choice));
        settled.current.add(result.done.journal);
        if (finished.outcome === "trashed") trashed += 1;
        else removed += 1;
        if (finished.warning) warnings.push(finished.warning);
      }
    } catch (error) {
      setBusy(false);
      const code = (error as { code?: string } | null)?.code;
      if (code === "no_trash") setStep(discarding ? "noTrashDiscard" : "noTrash");
      else setFailure(describeError(error));
      await queryClient.invalidateQueries();
      return;
    }
    setBusy(false);
    await queryClient.invalidateQueries();
    const done = discarding
      ? (trashed > 0
        ? plural(trashed, "The extracted folder was moved to the trash; the archive is unchanged.", "{count} extracted folders were moved to the trash; the archives are unchanged.")
        : plural(removed, "The extracted folder was removed; the archive is unchanged.", "{count} extracted folders were removed; the archives are unchanged."))
      : (trashed > 0
        ? plural(trashed, "The archive was moved to the trash.", "{count} archives were moved to the trash.")
        : plural(removed, "The archive was deleted.", "{count} archives were deleted."));
    onMessage([done, ...warnings].join(" "));
    if (discarding) onClose();
    else setStep("done");
  };
  const keep = () => {
    onMessage(t("Archives and folders kept. You can delete an archive later from its model page."));
    onClose();
  };
  const openFirst = () => {
    const folder = extracted[0]?.done?.folder;
    if (folder) onOpenFolder(library.id, folder);
    else onClose();
  };

  const percent = progress && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0;
  if (minimized && step === "run") {
    return (
      <button className="xpill" onClick={() => setMinimized(false)} title={t("Show the extraction")}>
        {t("Extracting {done} of {count}", { done: Math.min(results.length + 1, ready.length), count: ready.length })} · {percent} %
      </button>
    );
  }

  return (
    <div className="xdlg-back" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget && step !== "run" && !busy) onClose(); }}>
      <div className="xdlg" role="dialog" aria-modal="true" aria-label={t("Extract archives")}>
        {step === "plan" && (
          <>
            <h2>{items.length === 1 ? t("Extract “{name}”", { name: fileName(items[0]?.archive ?? "") }) : t("Extract {count} archives", { count: items.length })}</h2>
            <p className="muted">{t("Unpacks into a new folder next to the archive. Nothing is deleted now: you check the result first.")}</p>
            {plans.isPending && <p className="muted">{t("Reading archives…")}</p>}
            {plans.isError && <p className="err">{describeError(plans.error)}</p>}
            <div className="xlist">
              {(plans.data ?? []).map((plan) => (
                <div className={`xrow${plan.problem ? " bad" : ""}`} key={plan.archive}>
                  <div className="xname">
                    {items.length > 1 && !plan.problem && (
                      <input type="checkbox" checked={!skipped.has(plan.archive)} onChange={() => toggle(plan.archive)} aria-label={t("Extract {name}", { name: fileName(plan.archive) })} />
                    )}
                    <b>{fileName(plan.archive)}</b>
                    {plan.parts.length > 1 && <span className="pill">{plural(plan.parts.length, "{count} part", "{count} parts")}</span>}
                    <span className="muted">{plural(plan.files, "{count} file", "{count} files")} · {formatBytes(plan.bytes)}</span>
                  </div>
                  {plan.problem ? <p className="err">{plan.problem}</p> : skipped.has(plan.archive) ? null : (
                    <>
                      <label className="xfield">
                        <span>{t("New folder")}</span>
                        <input value={nameOf(plan)} onChange={(event) => setNames({ ...names, [plan.archive]: event.target.value })} aria-label={t("New folder")} />
                      </label>
                      {plan.clash && <p className="hint">{t("A folder with the archive's name already exists, so this one gets a number. Existing folders are never merged.")}</p>}
                      {plan.unwrapped && <p className="hint">{t("The archive holds one folder “{name}”. Its content goes straight into the new folder.", { name: plan.unwrapped })}</p>}
                    </>
                  )}
                </div>
              ))}
            </div>
            {plans.data && plans.data[0]?.free != null && <p className="muted">{t("Free on this drive: {size}", { size: formatBytes(plans.data[0].free) })}</p>}
            {needsPassword && (
              <form className="xfield" onSubmit={(event) => { event.preventDefault(); setPassword(typed || null); }}>
                <span>{t("Password")}</span>
                <input type="password" value={typed} onChange={(event) => setTyped(event.target.value)} aria-label={t("Password")} autoFocus />
                <button className="btn" type="submit">{t("Check")}</button>
              </form>
            )}
            <div className="xbar">
              <button className="btn ghost" onClick={onClose}>{t("Cancel")}</button>
              <button className="btn primary" disabled={ready.length === 0} onClick={() => void run()}>
                {ready.length === 1 ? t("Extract") : t("Extract {count} archives", { count: ready.length })}
              </button>
            </div>
          </>
        )}

        {step === "run" && (
          <>
            <h2>{t("Extracting {done} of {count}", { done: Math.min(results.length + 1, ready.length), count: ready.length })}</h2>
            <p className="muted">{current ? fileName(current) : ""}{progress?.file ? ` · ${progress.file}` : ""}</p>
            <div className="xprogress" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent}>
              <span style={{ width: `${percent}%` }} />
            </div>
            <p className="muted">{progress ? `${formatBytes(progress.done)} / ${formatBytes(progress.total)}` : t("Checking the archive…")}</p>
            <p className="hint">{t("Files are written to a hidden folder first. The new folder only appears once everything is unpacked and checked. Cancel leaves nothing behind.")}</p>
            <div className="xbar">
              <button className="btn" onClick={cancel}>{t("Cancel")}</button>
              <button className="btn primary" onClick={() => setMinimized(true)}>{t("Run in background")}</button>
            </div>
          </>
        )}

        {step === "review" && (
          <>
            <h2>{extracted.length > 0 ? t("Check the extracted files") : t("Nothing was extracted")}</h2>
            {extracted.map((result) => (
              <div className="xrow ok" key={result.plan.archive}>
                <div className="xname">
                  <b>{folderName(result.done?.folder ?? "")}</b>
                  <span className="muted">{plural(result.done?.files ?? 0, "{count} file", "{count} files")} · {formatBytes(result.done?.bytes ?? 0)} · {t("all checksums OK")}</span>
                  <button className="btn ghost small" onClick={() => onOpenFolder(library.id, result.done?.folder ?? "")}>{t("Show")}</button>
                </div>
                {(result.done?.pictures.length ?? 0) > 0 && (
                  <div className="xpics">{result.done?.pictures.map((picture) => <img key={picture} src={thumbUrl(library.id, picture)} alt="" loading="lazy" />)}</div>
                )}
                {result.done?.unwrapped && <p className="hint">{t("Unwrapped the single folder “{name}” inside.", { name: result.done.unwrapped })}</p>}
                {(result.done?.renamed.length ?? 0) > 0 && (
                  <details className="hint">
                    <summary>{plural(result.done?.renamed.length ?? 0, "{count} name adjusted: Windows and NAS shares cannot store it", "{count} names adjusted: Windows and NAS shares cannot store them")}</summary>
                    <ul>{result.done?.renamed.slice(0, 20).map((entry) => <li key={entry.from}>{entry.from} → {entry.to}</li>)}</ul>
                  </details>
                )}
                {(result.done?.skipped.length ?? 0) > 0 && <p className="hint">{plural(result.done?.skipped.length ?? 0, "{count} link in the archive was skipped.", "{count} links in the archive were skipped.")}</p>}
                {result.done?.warning && <p className="hint">{result.done.warning}</p>}
              </div>
            ))}
            {failed.map((result) => (
              <div className="xrow bad" key={result.plan.archive}>
                <div className="xname"><b>{fileName(result.plan.archive)}</b><span className="pill err">{t("failed")}</span></div>
                <p className="err">{result.error}</p>
                <p className="hint">{t("Nothing was written and the archive is untouched, so it is not offered for deletion.")}</p>
              </div>
            ))}
            {failure && <p className="err">{failure}</p>}
            {busy && <p className="muted" role="status">{t("Working… the folder is being read again.")}</p>}
            <div className="xbar">
              {extracted.length > 0 ? (
                <>
                  <button className="btn danger" disabled={busy} onClick={() => void finish("discard")}>{t("Discard the extracted folders")}</button>
                  <button className="btn" disabled={busy} onClick={keep}>{t("Keep archives and folders")}</button>
                  <button className="btn primary" disabled={busy} onClick={() => setStep("confirm")}>{plural(extracted.length, "Delete the archive…", "Delete the {count} archives…")}</button>
                </>
              ) : <button className="btn primary" onClick={onClose}>{t("Close")}</button>}
            </div>
          </>
        )}

        {(step === "confirm" || step === "noTrash") && (
          <>
            <h2>{step === "noTrash" ? t("This drive has no trash") : plural(partFiles.length, "Delete this archive file?", "Delete {count} archive files?")}</h2>
            <p className="muted">{t("The extracted folders stay. Tags and notes you gave the archives move to the new folders.")}</p>
            <ul className="xfiles">{partFiles.map((part) => <li key={part}>{fileName(part)}</li>)}</ul>
            {step === "noTrash" || network
              ? <p className="err"><b>{t("Deleted permanently.")}</b> {network ? t("This library is on a network share, which has no trash. This can't be undone.") : t("The archive could not be moved to the trash. Delete it permanently? This can't be undone.")}</p>
              : <p className="hint">{t("The files are moved to the trash.")}</p>}
            {failure && <p className="err">{failure}</p>}
            {busy && <p className="muted" role="status">{t("Deleting… then the folder is read again.")}</p>}
            <div className="xbar">
              <button className="btn ghost" disabled={busy} onClick={() => (pending ? onClose() : setStep("review"))}>{pending ? t("Cancel") : t("Back")}</button>
              <button className="btn primary" disabled={busy || extracted.length === 0} onClick={() => void finish(step === "noTrash" ? "deletePermanently" : "delete")}>
                {plural(partFiles.length, "Delete the file", "Delete {count} files")}
              </button>
            </div>
          </>
        )}

        {step === "noTrashDiscard" && (
          <>
            <h2>{t("This drive has no trash")}</h2>
            <p className="err"><b>{t("Removed permanently.")}</b> {t("The extracted folders cannot go to the trash here. Remove them for good? Files you added to them since are lost too. The archives stay.")}</p>
            <ul className="xfiles">{extracted.map((result) => <li key={result.plan.archive}>{folderName(result.done?.folder ?? "")}</li>)}</ul>
            {failure && <p className="err">{failure}</p>}
            {busy && <p className="muted" role="status">{t("Working… the folder is being read again.")}</p>}
            <div className="xbar">
              <button className="btn ghost" disabled={busy} onClick={() => setStep("review")}>{t("Back")}</button>
              <button className="btn danger" disabled={busy} onClick={() => void finish("discardPermanently")}>{t("Remove permanently")}</button>
            </div>
          </>
        )}

        {step === "done" && (
          <>
            <h2>{t("Done")}</h2>
            <p className="muted">{plural(extracted.length, "The archive was extracted and deleted; the library was rescanned.", "{count} archives were extracted and deleted; the library was rescanned.")}</p>
            <div className="xbar">
              <button className="btn" onClick={onClose}>{t("Close")}</button>
              {extracted.length === 1 && <button className="btn primary" onClick={openFirst}>{t("Open the new folder")}</button>}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

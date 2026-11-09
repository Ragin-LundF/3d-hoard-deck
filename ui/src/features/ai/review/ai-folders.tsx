import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ask } from "@tauri-apps/plugin-dialog";
import { commands, describeError, unwrap, type AiFoldersDto } from "../../../ipc";
import { Icon } from "../../../shared/icons";
import { formatNumber, plural, t } from "../../../shared/i18n";
import { useAiSettings, useSuggestProgress } from "../queries";
import { stateOf, toggle } from "./folder-choice";

/** Seconds per model with a local vision model (measured with LM Studio), for the estimate. */
const SECONDS_PER_MODEL = 3;

type Props = { library: number; onMessage: (message: string) => void };

/** The folders the AI works on in one library, how runs behave, and how many models that is. */
export function AiFolders({ library, onMessage }: Props) {
  const queryClient = useQueryClient();
  const saved = useQuery({ queryKey: ["aiFolders", library], queryFn: () => unwrap(commands.aiFolders(library)) });
  const [draft, setDraft] = useState<AiFoldersDto | null>(null);
  const folders = draft ?? saved.data;
  const remote = useAiSettings().data?.remote ?? true;
  const running = useSuggestProgress()?.running ?? false;
  const [count, setCount] = useState<number | null>(null);
  useEffect(() => {
    if (!folders) return;
    const timer = setTimeout(() => void unwrap(commands.countAiFolders(library, folders)).then(setCount, () => setCount(null)), 250);
    return () => clearTimeout(timer);
  }, [folders, library]);
  const save = useMutation({
    mutationFn: (next: AiFoldersDto) => unwrap(commands.saveAiFolders(library, next)),
    onSuccess: (_, next) => {
      queryClient.setQueryData(["aiFolders", library], next);
      setDraft(null);
    },
    onError: (error) => onMessage(describeError(error)),
  });
  const start = useMutation({
    mutationFn: async () => {
      if (remote && count && !(await ask(plural(count, "This sends {count} preview picture to the AI service. Cloud services charge for each one.", "This sends {count} preview pictures to the AI service. Cloud services charge for each one."), { kind: "warning", okLabel: t("Send"), cancelLabel: t("Cancel") }))) return null;
      return unwrap(commands.startAiFolders(library));
    },
    onSuccess: (total) => total !== null && onMessage(plural(total, "Suggesting tags for {count} model…", "Suggesting tags for {count} models…")),
    onError: (error) => onMessage(describeError(error)),
  });
  if (!folders) return null;
  const change = (next: AiFoldersDto) => setDraft(next);
  const minutes = count === null ? null : Math.max(1, Math.round((count * SECONDS_PER_MODEL) / 60));

  return (
    <div className="rv-folders">
      <div className="panel">
        <div className="prow2 stack">
          <div className="t"><b>{t("Folders the AI works on")}</b><span>{t("Tick a folder to include everything below it; untick a subfolder to leave it out.")}</span></div>
          <div className="ftree" role="tree"><FolderLevel library={library} parent="" folders={folders} onChange={change} /></div>
        </div>
        <label className="prow2"><div className="t"><b>{t("Only models without own tags")}</b></div>
          <input type="checkbox" checked={folders.onlyUntagged} onChange={(event) => change({ ...folders, onlyUntagged: event.target.checked })} /></label>
        <label className="prow2"><div className="t"><b>{t("Ask about new models after each scan")}</b><span>{t("After a library scan, models that are new in these folders get suggestions in the background.")}</span></div>
          <input type="checkbox" checked={folders.afterScan} onChange={(event) => change({ ...folders, afterScan: event.target.checked })} /></label>
        <label className="prow2"><div className="t"><b>{t("Skip models without a preview picture")}</b><span>{t("Otherwise the AI gets only their name and folder.")}</span></div>
          <input type="checkbox" checked={folders.skipWithoutPreview} onChange={(event) => change({ ...folders, skipWithoutPreview: event.target.checked })} /></label>
      </div>
      <div className="rv-foot">
        <span className="grow">
          {count === null ? "" : count === 0 ? t("No model waits for suggestions here.") : remote
            ? plural(count, "{count} model would be asked.", "{count} models would be asked.")
            : t("{count} models, about {minutes} min with a local model.", { count: formatNumber(count), minutes: minutes ?? 0 })}
        </span>
        {draft && <button className="btn ghost" onClick={() => setDraft(null)}>{t("Discard")}</button>}
        <button className="btn" disabled={!draft || save.isPending} onClick={() => save.mutate(folders)}>{t("Save")}</button>
        <button className="btn primary" disabled={draft !== null || running || !count || start.isPending} onClick={() => start.mutate()}><Icon name="tag" />{t("Suggest tags now")}</button>
      </div>
    </div>
  );
}

type LevelProps = { library: number; parent: string; folders: AiFoldersDto; onChange: (next: AiFoldersDto) => void };

function FolderLevel({ library, parent, folders, onChange }: LevelProps) {
  const children = useQuery({ queryKey: ["folderChildren", library, parent], queryFn: () => unwrap(commands.folderChildren(library, parent)) });
  const [open, setOpen] = useState<Set<string>>(new Set());
  return (
    <ul role="group">
      {(children.data ?? []).map((folder) => {
        const state = stateOf(folders, folder.path);
        const expanded = open.has(folder.path);
        return (
          <li key={folder.path} role="treeitem" aria-expanded={expanded} aria-selected={state === "on"}>
            <div className="frow">
              <button className="chev-btn" aria-label={expanded ? t("Collapse") : t("Expand")} onClick={() => { const next = new Set(open); if (expanded) next.delete(folder.path); else next.add(folder.path); setOpen(next); }}>
                <span className={`chev${expanded ? " open" : ""}`}><Icon name="chevron" /></span>
              </button>
              <input
                type="checkbox"
                checked={state === "on"}
                ref={(element) => { if (element) element.indeterminate = state === "mixed"; }}
                aria-label={folder.name}
                onChange={() => onChange(toggle(folders, folder.path))}
              />
              <span className="fname">{folder.name}</span>
              <span className="n">{formatNumber(folder.totalModels)}</span>
            </div>
            {expanded && <FolderLevel library={library} parent={folder.path} folders={folders} onChange={onChange} />}
          </li>
        );
      })}
    </ul>
  );
}

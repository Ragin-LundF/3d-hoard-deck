import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { commands, unwrap } from "../../ipc";
import { formatNumber, plural, t } from "../../shared/i18n";
import { useTags } from "../tags/queries";
import { useBatchTags, type BatchTarget } from "./batch";

type Props = {
  library: number;
  ids: number[];
  /** All results of the current search, when there is one; "Select all" then targets the search, not only loaded cards. */
  search: { query: string; folder: string; total: number } | null;
  all: boolean;
  onSelectAll: () => void;
  onClear: () => void;
  onMessage: (message: string) => void;
  /** Opens "Add tag" or "Remove tag" from outside (the card menu); a new object each time. */
  request?: { which: "add" | "remove" } | null;
};

/** Floating bar while models are selected: how many, select all, add a tag, remove tags, clear. */
export function SelectionBar({ library, ids, search, all, onSelectAll, onClear, onMessage, request }: Props) {
  const [open, setOpen] = useState<"add" | "remove" | null>(null);
  useEffect(() => {
    if (request) setOpen(request.which);
  }, [request]);
  const batch = useBatchTags(library, onMessage);
  const target: BatchTarget = all && search ? { query: search.query, folder: search.folder } : { ids };
  const count = all && search ? search.total : ids.length;
  const run = (add: string[], remove: string[]) =>
    batch.mutate({ target, add, remove }, { onSuccess: () => { setOpen(null); onClear(); } });

  return (
    <div className="selbar" role="toolbar" aria-label={t("Selected models")}>
      <b>{plural(count, "{count} selected", "{count} selected")}</b>
      {search && !all && search.total > ids.length && (
        <button className="linkish" onClick={onSelectAll}>{t("Select all {count}", { count: formatNumber(search.total) })}</button>
      )}
      <span className="spacer" />
      <div className="split">
        <button className="btn small" aria-expanded={open === "add"} onClick={() => setOpen(open === "add" ? null : "add")}>{t("Add tag")}</button>
        {open === "add" && <AddTag library={library} busy={batch.isPending} onAdd={(tag) => run([tag], [])} />}
      </div>
      <div className="split">
        <button className="btn small" aria-expanded={open === "remove"} onClick={() => setOpen(open === "remove" ? null : "remove")}>{t("Remove tag")}</button>
        {open === "remove" && <RemoveTags library={library} ids={ids} search={all ? search : null} count={count} busy={batch.isPending} onRemove={(tags) => run([], tags)} />}
      </div>
      <button className="btn ghost small" onClick={onClear}>{t("Clear")}</button>
    </div>
  );
}

function AddTag({ library, busy, onAdd }: { library: number; busy: boolean; onAdd: (tag: string) => void }) {
  const tags = useTags(library);
  const [text, setText] = useState("");
  return (
    <form className="selpop" onSubmit={(event) => { event.preventDefault(); if (text.trim()) onAdd(text.trim()); }}>
      <input autoFocus list="batch-tags" value={text} placeholder={t("Tag")} aria-label={t("Tag")} onChange={(event) => setText(event.target.value)} />
      <datalist id="batch-tags">{(tags.data ?? []).slice(0, 300).map((tag) => <option key={tag.name} value={tag.name} />)}</datalist>
      <button className="btn primary small" disabled={busy || !text.trim()}>{t("Add")}</button>
    </form>
  );
}

type RemoveProps = { library: number; ids: number[]; search: { query: string; folder: string } | null; count: number; busy: boolean; onRemove: (tags: string[]) => void };

/** The tags in the selection with how many of the models carry each; tick and remove. */
function RemoveTags({ library, ids, search, count, busy, onRemove }: RemoveProps) {
  const present = useQuery({
    queryKey: ["selection-tags", library, search ?? ids],
    queryFn: () => unwrap(search ? commands.tagFacets(library, search.folder, search.query, 60) : commands.selectionTags(library, ids)),
  });
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const list = present.data ?? [];
  return (
    <div className="selpop">
      {list.length === 0 ? <span className="hint">{t("The selected models have no tags.")}</span> : list.map((tag) => (
        <label key={tag.name} className="tr-check">
          <input type="checkbox" checked={picked.has(tag.name)} onChange={(event) => {
            const next = new Set(picked);
            if (event.target.checked) next.add(tag.name);
            else next.delete(tag.name);
            setPicked(next);
          }} />
          {tag.name} <span className="n">{formatNumber(tag.count)}/{formatNumber(count)}</span>
        </label>
      ))}
      <button className="btn primary small" disabled={busy || picked.size === 0} onClick={() => onRemove([...picked])}>{t("Remove")}</button>
    </div>
  );
}

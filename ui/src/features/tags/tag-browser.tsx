import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useMemo, useRef, useState } from "react";
import { commands, describeError, unwrap, type TagInfoDto_Serialize } from "../../ipc";
import { formatNumber, plural, t } from "../../shared/i18n";
import { Icon } from "../../shared/icons";
import { likelyDuplicates } from "./duplicates";
import { GROUP_COLORS, useTagGroups, useTags } from "./queries";
import { toggleExclude, toggleInclude } from "./tag-query";
import { menuProps } from "../../shared/context-menu";
import { takeRenameRequest, useTagMenu } from "./tag-menu";
import { SuggestionReview } from "../ai/suggestion-review";

type Sort = "count" | "name" | "recent";
/** Which group is shown: every tag, one group, or the tags without a group. */
type GroupFilter = "all" | "none" | number;
type Props = { library: number; libraryName: string; onSearch: (query: string) => void; onMessage: (message: string) => void };

const ROW = 36;
const LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ#".split("");
const letterOf = (name: string) => (/^[a-z]/i.test(name) ? name.charAt(0).toUpperCase() : "#");

/** Every tag of a library: filter, groups, sort, list or cloud, and bulk changes (merge, rename, group, pin, delete). */
export function TagBrowser({ library, libraryName, onSearch, onMessage }: Props) {
  const tags = useTags(library);
  const groups = useTagGroups(library);
  const queryClient = useQueryClient();
  const [filter, setFilter] = useState("");
  const [sort, setSort] = useState<Sort>("count");
  const [view, setView] = useState<"list" | "cloud">("list");
  const [group, setGroup] = useState<GroupFilter>("all");
  // "Rename…" chosen in a tag's menu elsewhere opens this page with that tag ready to rename.
  const [renameFirst] = useState(() => takeRenameRequest(library));
  const [selected, setSelected] = useState<Set<string>>(() => new Set(renameFirst ? [renameFirst] : []));
  const [action, setAction] = useState<"merge" | "rename" | "delete" | null>(renameFirst ? "rename" : null);
  const tagMenu = useTagMenu(onMessage);
  const menuOf = (name: string) =>
    tagMenu(library, name, {
      onShow: () => onSearch(toggleInclude("", name)),
      onExclude: () => onSearch(toggleExclude("", name)),
      onRename: () => {
        setSelected(new Set([name]));
        setAction("rename");
      },
    });
  const [editingGroup, setEditingGroup] = useState<{ id: number | null; name: string; color: string } | null>(null);
  const scroller = useRef<HTMLDivElement>(null);

  const all = tags.data ?? [];
  const colorOf = (id: number | null | undefined) => groups.data?.find((item) => item.id === id)?.color;
  const shown = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    const list = all.filter(
      (tag) =>
        (!needle || tag.name.toLowerCase().includes(needle)) &&
        (group === "all" || (group === "none" ? tag.group === null : tag.group === group)),
    );
    const by = { count: (a: TagInfoDto_Serialize, b: TagInfoDto_Serialize) => b.count - a.count, name: (a: TagInfoDto_Serialize, b: TagInfoDto_Serialize) => a.name.localeCompare(b.name), recent: (a: TagInfoDto_Serialize, b: TagInfoDto_Serialize) => (b.recent ?? 0) - (a.recent ?? 0) };
    return [...list].sort(by[sort]);
  }, [all, filter, group, sort]);
  const duplicates = useMemo(() => likelyDuplicates(all).slice(0, 3), [all]);
  const virtualizer = useVirtualizer({ count: shown.length, getScrollElement: () => scroller.current, estimateSize: () => ROW, overscan: 12 });

  const done = async (message?: string) => {
    setSelected(new Set());
    setAction(null);
    await queryClient.invalidateQueries();
    if (message) onMessage(message);
  };
  const run = useMutation({
    mutationFn: (work: () => Promise<string | undefined>) => work(),
    onSuccess: (message) => done(message),
    onError: (error) => onMessage(describeError(error)),
  });
  const names = [...selected];
  const merge = (into: string, from: string[]) =>
    run.mutate(async () => {
      const changed = await unwrap(commands.renameTags(library, from, into));
      return plural(changed, "{count} model updated.", "{count} models updated.");
    });
  const toggle = (name: string) => {
    const next = new Set(selected);
    if (next.has(name)) next.delete(name);
    else next.add(name);
    setSelected(next);
    setAction(null);
  };
  const jump = (letter: string) => {
    const index = shown.findIndex((tag) => letterOf(tag.name) === letter);
    if (index >= 0) virtualizer.scrollToIndex(index, { align: "start" });
  };
  const countIn = (id: GroupFilter) => all.filter((tag) => (id === "all" ? true : id === "none" ? tag.group === null : tag.group === id)).length;
  const maxCount = Math.max(1, ...shown.map((tag) => tag.count));

  return (
    <div className="tagbrowser">
      <div className="libhead">
        <div className="libtitle">
          <h1>{t("Tags")}</h1>
          <span className="crumbs">{libraryName} · {plural(all.length, "{count} tag", "{count} tags")}</span>
        </div>
        <div className="filters">
          <input className="tb-filter" value={filter} placeholder={t("Filter tags…")} aria-label={t("Filter tags…")} onChange={(event) => setFilter(event.target.value)} />
          <select className="sel" value={sort} aria-label={t("Sort by")} onChange={(event) => setSort(event.target.value as Sort)}>
            <option value="count">{t("Most used")}</option>
            <option value="name">{t("Name A–Z")}</option>
            <option value="recent">{t("Recently used")}</option>
          </select>
          <div className="seg" role="group" aria-label={t("Display")}>
            <button className={view === "list" ? "on" : ""} aria-pressed={view === "list"} onClick={() => setView("list")}>{t("List")}</button>
            <button className={view === "cloud" ? "on" : ""} aria-pressed={view === "cloud"} onClick={() => setView("cloud")}>{t("Cloud")}</button>
          </div>
        </div>
        {duplicates.length > 0 && (
          <div className="callout dupes">
            <Icon name="tag" />
            <span>{t("These look like the same tag:")}</span>
            {duplicates.map((same) => {
              const into = same[0]?.name ?? "";
              return (
                <button key={into} className="btn ghost" disabled={run.isPending} onClick={() => merge(into, same.map((tag) => tag.name))}>
                  {same.map((tag) => tag.name).join(" · ")} → {t("Merge into {name}", { name: into })}
                </button>
              );
            })}
          </div>
        )}
      </div>

      <SuggestionReview library={library} />

      <div className="tb-body">
        <nav className="tb-groups" aria-label={t("Tag groups")}>
          <button className={`nav${group === "all" ? " on" : ""}`} onClick={() => setGroup("all")}>
            <span className="lbl">{t("All tags")}</span><span className="n">{formatNumber(countIn("all"))}</span>
          </button>
          {groups.data?.map((item) => {
            const id = item.id ?? 0;
            return (
              <div className="tb-group" key={id}>
                <button className={`nav${group === id ? " on" : ""}`} onClick={() => setGroup(id)} onDoubleClick={() => setEditingGroup({ id, name: item.name, color: item.color })} title={t("Double-click to edit")}>
                  <span className="gd" style={{ background: item.color }} /><span className="lbl">{item.name}</span><span className="n">{formatNumber(countIn(id))}</span>
                </button>
              </div>
            );
          })}
          {(groups.data?.length ?? 0) > 0 && (
            <button className={`nav${group === "none" ? " on" : ""}`} onClick={() => setGroup("none")}>
              <span className="gd" /><span className="lbl">{t("Other")}</span><span className="n">{formatNumber(countIn("none"))}</span>
            </button>
          )}
          {editingGroup ? (
            <form
              className="tb-groupform"
              onSubmit={(event) => {
                event.preventDefault();
                const editing = editingGroup;
                run.mutate(async () => {
                  await unwrap(commands.saveTagGroup(library, editing.id, editing.name, editing.color));
                  setEditingGroup(null);
                  return undefined;
                });
              }}
            >
              <input autoFocus value={editingGroup.name} placeholder={t("Group name")} aria-label={t("Group name")} onChange={(event) => setEditingGroup({ ...editingGroup, name: event.target.value })} />
              <div className="swatches" role="radiogroup" aria-label={t("Colour")}>
                {GROUP_COLORS.map((color) => (
                  <button key={color} type="button" role="radio" aria-checked={editingGroup.color === color} aria-label={color} className={editingGroup.color === color ? "on" : ""} style={{ background: color }} onClick={() => setEditingGroup({ ...editingGroup, color })} />
                ))}
              </div>
              <div className="actions">
                <button className="btn primary" type="submit" disabled={!editingGroup.name.trim()}>{t("Save")}</button>
                {editingGroup.id !== null && (
                  <button
                    className="btn ghost"
                    type="button"
                    onClick={() => {
                      const id = editingGroup.id ?? 0;
                      setEditingGroup(null);
                      if (group === id) setGroup("all");
                      run.mutate(async () => {
                        await unwrap(commands.deleteTagGroup(library, id));
                        return undefined;
                      });
                    }}
                  >
                    {t("Delete group")}
                  </button>
                )}
                <button className="btn ghost" type="button" onClick={() => setEditingGroup(null)}>{t("Cancel")}</button>
              </div>
            </form>
          ) : (
            <button className="nav" onClick={() => setEditingGroup({ id: null, name: "", color: GROUP_COLORS[0] ?? "#4f8cff" })}>
              <Icon name="plus" /><span className="lbl">{t("New group")}</span>
            </button>
          )}
        </nav>

        {view === "list" ? (
          <div className="tb-listwrap">
            <div className="tb-list" ref={scroller} role="list" aria-label={t("Tags")}>
              {shown.length === 0 && <p className="hint">{t("No tag matches.")}</p>}
              <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
                {virtualizer.getVirtualItems().map((row) => {
                  const tag = shown[row.index];
                  if (!tag) return null;
                  const isOn = selected.has(tag.name);
                  return (
                    <div key={tag.name} role="listitem" className={`tb-row${isOn ? " on" : ""}`} style={{ position: "absolute", top: row.start, height: ROW, left: 0, right: 0 }} {...menuProps(() => menuOf(tag.name))}>
                      <input type="checkbox" checked={isOn} onChange={() => toggle(tag.name)} aria-label={t("Select {name}", { name: tag.name })} />
                      <span className="gd" style={{ background: colorOf(tag.group) ?? "transparent" }} />
                      <button className="linkish name" onClick={() => onSearch(toggleInclude("", tag.name))} title={t("Show models with this tag")}>{tag.name}</button>
                      <span className="n">{formatNumber(tag.count)}</span>
                      <button
                        className={`iconbtn${tag.pinned ? " fav" : ""}`}
                        aria-pressed={tag.pinned}
                        aria-label={tag.pinned ? t("Unpin {name}", { name: tag.name }) : t("Pin {name}", { name: tag.name })}
                        title={tag.pinned ? t("Unpin from the sidebar") : t("Pin to the sidebar")}
                        onClick={() => run.mutate(async () => {
                          await unwrap(commands.setTagsPinned(library, [tag.name], !tag.pinned));
                          return undefined;
                        })}
                      >
                        <Icon name={tag.pinned ? "pinFilled" : "pin"} />
                      </button>
                    </div>
                  );
                })}
              </div>
            </div>
            <div className="rail" aria-label={t("Jump to letter")}>
              {LETTERS.map((letter) => {
                const has = shown.some((tag) => letterOf(tag.name) === letter);
                return <button key={letter} className={has ? "" : "off"} disabled={!has} onClick={() => jump(letter)}>{letter}</button>;
              })}
            </div>
          </div>
        ) : (
          <div className="cloud" aria-label={t("Tags")}>
            {shown.map((tag) => (
              <button
                key={tag.name}
                className={selected.has(tag.name) ? "on" : ""}
                style={{ fontSize: `${12 + Math.round(12 * Math.sqrt(tag.count / maxCount))}px`, color: colorOf(tag.group) }}
                onClick={(event) => (event.metaKey || event.ctrlKey || event.shiftKey ? toggle(tag.name) : onSearch(toggleInclude("", tag.name)))}
                {...menuProps(() => menuOf(tag.name))}
                title={t("{count} models. ⌘-click to select", { count: tag.count })}
              >
                {tag.name}
              </button>
            ))}
          </div>
        )}
      </div>

      {names.length > 0 && (
        <div className="bulk" role="toolbar" aria-label={t("Selected tags")}>
          <span className="c">{plural(names.length, "{count} tag selected", "{count} tags selected")}</span>
          {action === null && (
            <>
              {names.length > 1 && <button className="btn" onClick={() => setAction("merge")}>{t("Merge…")}</button>}
              {names.length === 1 && <button className="btn" onClick={() => setAction("rename")}>{t("Rename…")}</button>}
              <select
                className="sel"
                value=""
                aria-label={t("Move to group")}
                onChange={(event) => {
                  const value = event.target.value;
                  run.mutate(async () => {
                    await unwrap(commands.setTagsGroup(library, names, value === "none" ? null : Number(value)));
                    return undefined;
                  });
                }}
              >
                <option value="" disabled>{t("Move to group…")}</option>
                {groups.data?.map((item) => <option key={item.id} value={item.id ?? 0}>{item.name}</option>)}
                <option value="none">{t("No group")}</option>
              </select>
              <button className="btn" onClick={() => run.mutate(async () => {
                await unwrap(commands.setTagsPinned(library, names, true));
                return undefined;
              })}>{t("Pin")}</button>
              <button className="btn" onClick={() => run.mutate(async () => {
                await unwrap(commands.setTagsPinned(library, names, false));
                return undefined;
              })}>{t("Unpin")}</button>
              <button className="btn danger" onClick={() => setAction("delete")}>{t("Delete…")}</button>
              <button className="btn ghost" onClick={() => setSelected(new Set())}>{t("Clear")}</button>
            </>
          )}
          {(action === "merge" || action === "rename") && (
            <form
              className="bulk-form"
              onSubmit={(event) => {
                event.preventDefault();
                const into = new FormData(event.currentTarget).get("into")?.toString().trim() ?? "";
                if (into) merge(into, names);
              }}
            >
              <input
                name="into"
                autoFocus
                defaultValue={action === "rename" ? names[0] : [...all].filter((tag) => selected.has(tag.name)).sort((a, b) => b.count - a.count)[0]?.name}
                aria-label={action === "merge" ? t("Merge into") : t("New name")}
              />
              <button className="btn primary" type="submit" disabled={run.isPending}>{action === "merge" ? t("Merge") : t("Rename")}</button>
              <button className="btn ghost" type="button" onClick={() => setAction(null)}>{t("Cancel")}</button>
            </form>
          )}
          {action === "delete" && (
            <>
              <span>{t("Remove these tags from every model? The models stay.")}</span>
              <button
                className="btn danger"
                disabled={run.isPending}
                onClick={() => run.mutate(async () => {
                  const changed = await unwrap(commands.deleteTags(library, names));
                  return plural(changed, "{count} model updated.", "{count} models updated.");
                })}
              >
                {t("Delete")}
              </button>
              <button className="btn ghost" onClick={() => setAction(null)}>{t("Cancel")}</button>
            </>
          )}
        </div>
      )}
    </div>
  );
}

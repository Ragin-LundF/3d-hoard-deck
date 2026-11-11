import { useCallback, useEffect, useMemo, useState, type MouseEvent } from "react";
import { commands, describeError, unwrap, type FolderDto_Serialize, type LibraryDto, type ModelSummaryDto_Serialize, type SortDto } from "../../ipc";
import { folderName } from "../../shared/format";
import { Icon } from "../../shared/icons";
import type { LibraryRoute, Route } from "../../shared/route";
import { Breadcrumbs } from "../folders/breadcrumbs";
import { useFolderMenu, useLibraryMenu } from "../folders/folder-menu";
import { useModelMenu } from "./model-menu";
import { openContextMenu } from "../../shared/context-menu";
import { NameInput } from "../collections/name-input";
import { useCreateCollection } from "../collections/queries";
import { FolderCard, ModelCard } from "./cards";
import { FolderPictures } from "./folder-pictures";
import { TagBar } from "../tags/tag-bar";
import { useFolderPage, useSearch } from "./queries";
import { VirtualGrid } from "./virtual-grid";
import { SuggestFolderButton } from "../ai/suggest-folder-button";
import { SelectionBar } from "../batch/selection-bar";
import { RenderedStrip } from "../render/rendered-strip";
import { FolderArchives, isArchive } from "../files/archive-actions";
import { useBatchTags } from "../batch/batch";
import { ask } from "@tauri-apps/plugin-dialog";
import { t, plural } from "../../shared/i18n";

type Item = { kind: "folder"; folder: FolderDto_Serialize } | { kind: "model"; model: ModelSummaryDto_Serialize };

/** "Save as smart collection" for the current search. */
function SaveSearch({ query, library, scope, onError, onSaved }: { query: string; library: number; scope: string; onError: (message: string) => void; onSaved: (id: number) => void }) {
  const [naming, setNaming] = useState(false);
  const create = useCreateCollection(onError, onSaved);
  if (naming) {
    return <NameInput placeholder={t("Smart collection name")} initial={query} onSubmit={(name) => { setNaming(false); create.mutate({ name, query, library, scope: scope || null }); }} onCancel={() => setNaming(false)} />;
  }
  return <button className="btn ghost" onClick={() => setNaming(true)}><Icon name="search" />{t("Save as smart collection")}</button>;
}

type Props = {
  library: LibraryDto;
  route: LibraryRoute;
  navigate: (route: Route) => void;
  onShownChange: (shown: number) => void;
  onError: (message: string) => void;
};

export function LibraryView({ library, route, navigate, onShownChange, onError }: Props) {
  const folderMenu = useFolderMenu(onError);
  const libraryMenu = useLibraryMenu(onError);
  const crumbMenu = (folder: string) => (folder ? folderMenu(library, folder) : libraryMenu(library));
  const modelMenu = useModelMenu(onError);
  const [selectionRequest, setSelectionRequest] = useState<{ which: "add" | "remove" } | null>(null);
  const [sort, setSort] = useState<SortDto>("name");
  const [cardWidth, setCardWidth] = useState(208);
  const searching = route.q.trim().length > 0;
  const browse = useFolderPage(searching ? null : library.id, route.folder, route.all, sort);
  const search = useSearch(searching ? library.id : null, route.q, route.folder);
  const active = searching ? search : browse;

  const items = useMemo<Item[]>(() => {
    if (searching) return (search.data?.pages ?? []).flatMap((page) => page.models.map((model) => ({ kind: "model" as const, model })));
    const pages = browse.data?.pages ?? [];
    const folders = pages.flatMap((page) => page.folders.map((folder) => ({ kind: "folder" as const, folder })));
    return [...folders, ...pages.flatMap((page) => page.models.map((model) => ({ kind: "model" as const, model })))];
  }, [searching, search.data, browse.data]);
  const total = searching ? search.data?.pages[0]?.total : undefined;
  useEffect(() => onShownChange(items.length), [items.length, onShownChange]);

  const { hasNextPage, isFetchingNextPage, fetchNextPage } = active;
  const loadMore = useCallback(() => {
    if (hasNextPage && !isFetchingNextPage) void fetchNextPage();
  }, [hasNextPage, isFetchingNextPage, fetchNextPage]);

  // Like the sidebar: a folder always opens in folder view; "Everything inside" is chosen per folder.
  const openFolder = (folder: string) => navigate({ ...route, folder, q: "", all: false });

  const batch = useBatchTags(library.id, onError);
  const removeEverywhere = async (tag: string) => {
    const count = total ?? 0;
    const yes = await ask(t("Remove “{tag}” from all {count} models shown here? Tags from a tag rule are hidden on them; the rule stays.", { tag, count }), {
      title: t("Remove tag"), kind: "warning", okLabel: t("Remove"), cancelLabel: t("Cancel"),
    });
    if (yes) batch.mutate({ target: { query: route.q, folder: route.folder }, add: [], remove: [tag] });
  };

  // Multi-select for batch tag changes: ⌘/Ctrl-click toggles, ⇧-click selects a range, ⌘A all loaded, Esc clears.
  const modelIds = useMemo(() => items.flatMap((item) => (item.kind === "model" ? [item.model.id] : [])), [items]);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [allResults, setAllResults] = useState(false);
  const [anchor, setAnchor] = useState<number | null>(null);
  const [selectMode, setSelectMode] = useState(false);
  const selecting = selectMode || selected.size > 0;
  const clearSelection = useCallback(() => { setSelected(new Set()); setAllResults(false); setSelectMode(false); }, []);
  useEffect(() => clearSelection(), [route.folder, route.q, route.all, clearSelection]);
  useEffect(() => {
    const keys = (event: KeyboardEvent) => {
      if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return;
      if (event.key === "Escape" && selecting) clearSelection();
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "a" && modelIds.length > 0) {
        event.preventDefault();
        setSelected(new Set(modelIds));
      }
    };
    window.addEventListener("keydown", keys);
    return () => window.removeEventListener("keydown", keys);
  }, [selecting, modelIds, clearSelection]);
  const select = (id: number) => (event: MouseEvent) => {
    const next = new Set(selected);
    const index = modelIds.indexOf(id);
    if (event.shiftKey && anchor !== null) {
      const [from, to] = [Math.min(anchor, index), Math.max(anchor, index)];
      modelIds.slice(from, to + 1).forEach((modelId) => next.add(modelId));
    } else if (next.has(id)) next.delete(id);
    else next.add(id);
    setSelected(next);
    setAllResults(false);
    setAnchor(index);
  };
  const title = folderName(route.folder) || library.name;

  return (
    <>
      <div className="libhead">
        <Breadcrumbs libraryName={library.name} folder={route.folder} onOpen={openFolder} menu={crumbMenu} />
        <div className="libtitle">
          <h1>{searching ? t("Search: {query}", { query: route.q }) : title}</h1>
          <span className="crumbs">
            {searching && total !== undefined ? plural(total, "{count} match", "{count} matches") : null}
          </span>
          <span className="spacer" />
          {!searching && route.folder && (
            <button
              className="btn ghost"
              title={t("Reads only this folder and its subfolders again, much faster than the whole library")}
              onClick={() => unwrap(commands.scanFolder(library.id, route.folder)).catch((error) => onError(describeError(error)))}
            >
              <Icon name="refresh" />{t("Scan this folder")}
            </button>
          )}
          {!searching && route.folder && <SuggestFolderButton library={library.id} folder={route.folder} onMessage={onError} />}
          {!searching && (
            <div className="seg" role="group" aria-label={t("Folder display")}>
              <button className={route.all ? "" : "on"} aria-pressed={!route.all} onClick={() => navigate({ ...route, all: false })}>
                <Icon name="folder" />{t("Folders")}
              </button>
              <button className={route.all ? "on" : ""} aria-pressed={route.all} onClick={() => navigate({ ...route, all: true })}>
                <Icon name="layers" />{t("Everything inside")}
              </button>
            </div>
          )}
        </div>
        <div className="filters">
          {searching && (
            <SaveSearch query={route.q} library={library.id} scope={route.folder} onError={onError} onSaved={(id) => navigate({ view: "collection", id })} />
          )}
          {searching && route.folder && (
            <span className="scope">
              {t("in {folder}", { folder: folderName(route.folder) })}
              <button aria-label={t("Search everywhere")} onClick={() => navigate({ ...route, folder: "" })}>
                <Icon name="close" />
              </button>
            </span>
          )}
          <span className="spacer" />
          {modelIds.length > 0 && (
            <button className={`btn ghost${selecting ? " on" : ""}`} onClick={() => (selecting ? clearSelection() : setSelectMode(true))} title={t("Select models to add or remove tags on all of them")}>
              <Icon name="check" />{selecting ? t("Done") : t("Select")}
            </button>
          )}
          {!searching && (
            <select className="sel" aria-label={t("Sort by")} value={sort} onChange={(event) => setSort(event.target.value as SortDto)}>
              <option value="name">{t("Name A–Z")}</option>
              <option value="added">{t("Recently added")}</option>
              <option value="released">{t("Release date")}</option>
            </select>
          )}
          <label className="range" title={t("Card size")}>
            <input type="range" min={150} max={320} value={cardWidth} aria-label={t("Card size")} onChange={(event) => setCardWidth(Number(event.target.value))} />
          </label>
        </div>
        <TagBar
          library={library.id}
          folder={route.folder}
          query={route.q}
          onQuery={(q) => navigate({ ...route, q })}
          onCheck={(tag) => navigate({ view: "check", library: library.id, tag, folder: route.folder })}
          onRemoveAll={(tag) => void removeEverywhere(tag)}
        />
      </div>
      <RenderedStrip
        library={library}
        ids={items.flatMap((item) => (item.kind === "model" && !item.model.cover && item.model.rendered === "cached" ? [item.model.id] : []))}
        onMessage={onError}
      />
      {!searching && !route.all && (
        <FolderArchives
          library={library}
          archives={items.flatMap((item) => (item.kind === "model" && item.model.formats.length === 0 && isArchive(item.model.path) ? [{ id: item.model.id, path: item.model.path, size: item.model.totalSize ?? 0 }] : []))}
        />
      )}
      {!searching && !route.all && <FolderPictures library={library.id} images={browse.data?.pages[0]?.images ?? []} />}
      {active.isError ? (
        <div className="empty err">{describeError(active.error)}</div>
      ) : active.isPending && (searching || browse.fetchStatus !== "idle") ? (
        <div className="empty">{t("Loading…")}</div>
      ) : items.length === 0 ? (
        <div className="empty">{searching ? t("Nothing found. Typos are fine, try fewer words.") : t("No models here yet.")}</div>
      ) : (
        <VirtualGrid
          items={items}
          cardWidth={cardWidth}
          itemKey={(item) => (item.kind === "folder" ? `f:${item.folder.path}` : `m:${item.model.id}`)}
          onNearEnd={loadMore}
          renderItem={(item) =>
            item.kind === "folder" ? (
              <FolderCard
                library={library.id}
                folder={item.folder}
                onOpen={() => (item.folder.isCollection ? navigate({ ...route, folder: item.folder.path, all: true, q: "" }) : openFolder(item.folder.path))}
                onMenu={(event) => openContextMenu(event, folderMenu(library, item.folder.path, item.folder.isCollection))}
              />
            ) : (
              <ModelCard model={item.model} onOpen={() => navigate({ view: "model", id: item.model.id })} selected={allResults || selected.has(item.model.id)} selecting={selecting} onSelect={select(item.model.id)} canWrite={library.access !== "readOnly"} onMessage={onError}
                onMenu={(event) => openContextMenu(event, (allResults || selected.has(item.model.id))
                  // A right-click on a selected card acts on the whole selection.
                  ? [
                      { label: plural(allResults && total !== undefined ? total : selected.size, "Add tag to {count} model…", "Add tag to {count} models…"), onSelect: () => setSelectionRequest({ which: "add" }) },
                      { label: plural(allResults && total !== undefined ? total : selected.size, "Remove tag from {count} model…", "Remove tag from {count} models…"), onSelect: () => setSelectionRequest({ which: "remove" }) },
                      "separator",
                      { label: t("Clear selection"), onSelect: clearSelection },
                    ]
                  : modelMenu(item.model, { onSelect: () => { setSelectMode(true); setSelected(new Set([...selected, item.model.id])); } }))} />
            )
          }
        />
      )}
      {(selected.size > 0 || allResults) && (
        <SelectionBar
          library={library.id}
          ids={[...selected]}
          search={searching && total !== undefined ? { query: route.q, folder: route.folder, total } : null}
          all={allResults}
          onSelectAll={() => { setAllResults(true); setSelected(new Set(modelIds)); }}
          onClear={clearSelection}
          onMessage={onError}
          request={selectionRequest}
        />
      )}
    </>
  );
}

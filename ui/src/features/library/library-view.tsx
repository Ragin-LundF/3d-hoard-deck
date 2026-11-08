import { useCallback, useEffect, useMemo, useState, type MouseEvent } from "react";
import { describeError, type FolderDto_Serialize, type LibraryDto, type ModelSummaryDto_Serialize, type SortDto } from "../../ipc";
import { folderName } from "../../shared/format";
import { Icon } from "../../shared/icons";
import type { LibraryRoute, Route } from "../../shared/route";
import { Breadcrumbs } from "../folders/breadcrumbs";
import { NameInput } from "../collections/name-input";
import { useCreateCollection } from "../collections/queries";
import { FolderCard, ModelCard } from "./cards";
import { FolderPictures } from "./folder-pictures";
import { useFolderPage, useSearch } from "./queries";
import { VirtualGrid } from "./virtual-grid";
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
  onFolderMenu: (folder: FolderDto_Serialize) => (event: MouseEvent) => void;
  onShownChange: (shown: number) => void;
  onError: (message: string) => void;
};

export function LibraryView({ library, route, navigate, onFolderMenu, onShownChange, onError }: Props) {
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

  const openFolder = (folder: string) => navigate({ ...route, folder, q: "" });
  const title = folderName(route.folder) || library.name;

  return (
    <>
      <div className="libhead">
        <Breadcrumbs libraryName={library.name} folder={route.folder} onOpen={openFolder} />
        <div className="libtitle">
          <h1>{searching ? t("Search: {query}", { query: route.q }) : title}</h1>
          <span className="crumbs">
            {searching && total !== undefined ? plural(total, "{count} match", "{count} matches") : null}
          </span>
          <span className="spacer" />
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
      </div>
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
                onMenu={onFolderMenu(item.folder)}
              />
            ) : (
              <ModelCard model={item.model} onOpen={() => navigate({ view: "model", id: item.model.id })} />
            )
          }
        />
      )}
    </>
  );
}

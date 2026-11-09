import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";
import { commands, describeError, unwrap, type IpcFailure, type LibraryDto } from "./ipc";
import { FolderMenu, useFolderMenu } from "./features/folders/folder-menu";
import { FolderTree } from "./features/folders/folder-tree";
import { LibraryNav } from "./features/organize/library-nav";
import { PinnedTags } from "./features/tags/pinned-tags";
import { TagBrowser } from "./features/tags/tag-browser";
import { CollectionsNav } from "./features/collections/collections-nav";
import { CollectionView } from "./features/collections/collection-view";
import { DuplicatesView } from "./features/duplicates/duplicates-view";
import { LibraryView } from "./features/library/library-view";
import { useLibraries } from "./features/library/queries";
import { isScanning, useScanStatus, type ScanStatus } from "./features/library/use-scan-status";
import { ModelPage } from "./features/model/model-page";
import { SearchBox } from "./features/search/search-box";
import { SettingsView, useAddLibrary } from "./features/settings/settings-view";
import { Icon, Logo } from "./shared/icons";
import { HOME, settingsRoute, useRoute } from "./shared/route";
import { useTheme } from "./shared/use-theme";
import { SuggestStatus } from "./features/ai/suggest-status";
import { UpdatePill } from "./features/update/update-pill";
import { useUpdateCheck } from "./features/update/use-update";
import { useSuggestProgressListener } from "./features/ai/queries";
import { SettingsNav } from "./features/settings/settings-nav";
import { t, plural } from "./shared/i18n";

export function App() {
  const [route, navigate] = useRoute();
  const [theme, toggleTheme, setTheme] = useTheme();
  const [toast, setToast] = useState<string | null>(null);
  const [shown, setShown] = useState(0);
  const libraries = useLibraries();
  const onScanFailed = useCallback((error: IpcFailure) => setToast(describeError(error)), []);
  const scans = useScanStatus(onScanFailed);
  useSuggestProgressListener();
  useUpdateCheck();
  const menu = useFolderMenu();
  const addLibrary = useAddLibrary(setToast);

  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(null), 5000);
    return () => clearTimeout(timer);
  }, [toast]);

  const list = libraries.data ?? [];
  const libraryRoute = route.view === "library" ? route : null;
  const library = list.find((candidate) => candidate.id === libraryRoute?.library) ?? list[0];
  const onSearch = useCallback(
    (q: string) => navigate({ ...HOME, ...(libraryRoute ?? {}), library: library?.id ?? null, q, view: "library" }, { replace: (libraryRoute?.q ?? "") !== "" && q !== "" }),
    [navigate, libraryRoute, library],
  );
  const rescan = (target: LibraryDto) => unwrap(commands.scanLibrary(target.id, false)).catch((error) => setToast(describeError(error)));

  return (
    <div className="win">
      <header className="titlebar">
        <div className="brand"><Logo /><span><b>3D</b> Hoard Deck</span></div>
        <SearchBox value={libraryRoute?.q ?? ""} onSearch={onSearch} />
        <div className="title-actions">
          <UpdatePill scanning={list.some((item) => isScanning(scans[item.id]))} />
          <button className="tbtn" onClick={toggleTheme} aria-label={theme === "dark" ? t("Switch to light theme") : t("Switch to dark theme")} title={t("Light / dark theme")}>
            <Icon name={theme === "dark" ? "sun" : "moon"} />
          </button>
          <button className="tbtn" onClick={() => library && void rescan(library)} disabled={!library || isScanning(scans[library.id])} aria-label={t("Scan for changes")} title={t("Scan for changes")}>
            <Icon name="refresh" />
          </button>
          <button className="tbtn" onClick={() => navigate(settingsRoute())} aria-label={t("Settings")} title={t("Settings")}>
            <Icon name="settings" />
          </button>
        </div>
      </header>

      <aside className="side" aria-label={t("Sidebar")}>
        {route.view === "settings" ? (
          <SettingsNav route={route} libraries={list} scans={scans} onNavigate={navigate} onBack={() => (history.length > 1 ? history.back() : navigate(HOME))} />
        ) : (
        <div className="side-scroll">
          {list.map((item) => (
            <div className="sec" key={item.id}>
              <h3>{item.name}</h3>
              <LibraryNav
                library={item.id}
                route={libraryRoute && item.id === library?.id ? libraryRoute : null}
                onNavigate={(patch) => navigate({ ...HOME, library: item.id, ...patch })}
                onTags={() => navigate({ view: "tags", library: item.id })}
                tagsOpen={route.view === "tags" && route.library === item.id}
              />
              <h4 className="subhead">{t("Folders")}</h4>
              <FolderTree
                library={item.id}
                current={libraryRoute && item.id === library?.id ? libraryRoute.folder : null}
                onOpen={(folder) => navigate({ ...HOME, library: item.id, folder })}
                onMenu={(folder) => menu.open(item.id, folder.path, folder.isCollection)}
              />
              <PinnedTags
                library={item.id}
                query={libraryRoute && item.id === library?.id ? libraryRoute.q : null}
                onSearch={(q) => navigate({ ...HOME, ...(libraryRoute && item.id === library?.id ? libraryRoute : {}), library: item.id, q, view: "library" })}
              />
            </div>
          ))}
          <CollectionsNav current={route.view === "collection" ? route.id : null} onOpen={(id) => navigate({ view: "collection", id })} onError={setToast} />
          <div className="sec">
            <button className={`nav${route.view === "duplicates" ? " on" : ""}`} onClick={() => navigate({ view: "duplicates" })}>
              <Icon name="layers" />{t("Duplicates")}
            </button>
          </div>
        </div>
        )}
        {library && route.view !== "settings" && <SourceCard library={library} scan={scans[library.id]} onSettings={() => navigate(settingsRoute("library", library.id, "access"))} />}
      </aside>

      <main className="main">
        {libraries.isError ? (
          <div className="empty err">{describeError(libraries.error)}</div>
        ) : libraries.isPending ? null : route.view === "settings" ? (
          <SettingsView route={route} navigate={navigate} libraries={list} scans={scans} theme={theme} onTheme={setTheme} onTags={(id) => navigate({ view: "tags", library: id })} onError={setToast} />
        ) : route.view === "tags" ? (
          <TagBrowser
            library={route.library}
            libraryName={list.find((item) => item.id === route.library)?.name ?? ""}
            onSearch={(q) => navigate({ ...HOME, library: route.library, q })}
            onMessage={setToast}
          />
        ) : route.view === "duplicates" ? (
          <DuplicatesView libraries={list} onOpenModel={(id) => navigate({ view: "model", id })} />
        ) : route.view === "collection" ? (
          <CollectionView id={route.id} onOpenModel={(id) => navigate({ view: "model", id })} onDeleted={() => navigate(HOME)} onError={setToast} />
        ) : route.view === "model" ? (
          <ModelPage id={route.id} libraries={list} onBack={() => history.back()} onOpenFolder={(id, folder) => navigate({ ...HOME, library: id, folder, all: true })} onMessage={setToast} />
        ) : library && libraryRoute ? (
          <LibraryView
            key={library.id}
            library={library}
            route={{ ...libraryRoute, library: library.id }}
            navigate={navigate}
            onFolderMenu={(folder) => menu.open(library.id, folder.path, folder.isCollection)}
            onShownChange={setShown}
            onError={setToast}
          />
        ) : (
          <div className="empty">
            <h2>{t("Add your first library")}</h2>
            <span>{t("Pick the folder with your STL and 3MF files. The files stay where they are.")}</span>
            <button className="btn primary" onClick={() => addLibrary.mutate()} disabled={addLibrary.isPending}>
              <Icon name="plus" />{t("Add library folder")}
            </button>
          </div>
        )}
      </main>

      <footer className="status">
        {route.view === "library" && <span>{t("{count} shown", { count: shown })}</span>}
        <span>{plural(list.length, "{count} library", "{count} libraries")}</span>
        <SuggestStatus />
      </footer>

      {menu.target && <FolderMenu target={menu.target} onClose={menu.close} onError={setToast} />}
      {toast && <div className="toast" role="status">{toast}</div>}
    </div>
  );
}

function SourceCard({ library, scan, onSettings }: { library: LibraryDto; scan: ScanStatus | undefined; onSettings: () => void }) {
  const scanning = isScanning(scan);
  const proposals = useQuery({ queryKey: ["presets", library.id], queryFn: () => unwrap(commands.proposePresets(library.id)) });
  const suggestions = proposals.data?.length ?? 0;
  const label = scan?.phase === "failed" ? t("Scan failed") : scanning ? (scan?.phase === "grouping" ? t("Grouping models…") : t("Scanning…")) : t("Up to date");
  return (
    <div className="source">
      <div className="row">
        <Icon name="drive" />
        <b style={{ fontWeight: 500 }}>{library.name}</b>
      </div>
      <div className="path">{library.root}</div>
      {scanning && <div className="bar"><i /></div>}
      <div className="scanline">
        <span className={scan?.phase === "failed" ? "err" : undefined}>{label}</span>
        {scanning && scan && <span className="mono">{plural(scan.files, "{count} file", "{count} files")}</span>}
      </div>
      {!scanning && (scan?.unreadable ?? 0) > 0 && (
        <span className="err" title={scan?.unreadableExamples.join("\n")}>
          {plural(scan?.unreadable ?? 0, "{count} folder or file could not be read", "{count} folders or files could not be read")}
        </span>
      )}
      {suggestions > 0 && !scanning && (
        <button className="linkish" onClick={onSettings}>
          {plural(suggestions, "{count} collection suggestion", "{count} collection suggestions")} ›
        </button>
      )}
    </div>
  );
}

import { useCallback, useEffect, useState } from "react";
import { commands, describeError, unwrap, type IpcFailure, type LibraryDto } from "./ipc";
import { FolderMenu, useFolderMenu } from "./features/folders/folder-menu";
import { FolderTree } from "./features/folders/folder-tree";
import { LibraryView } from "./features/library/library-view";
import { useLibraries } from "./features/library/queries";
import { isScanning, useScanStatus, type ScanStatus } from "./features/library/use-scan-status";
import { ModelPage } from "./features/model/model-page";
import { SearchBox } from "./features/search/search-box";
import { SettingsView, useAddLibrary } from "./features/settings/settings-view";
import { Icon, Logo } from "./shared/icons";
import { HOME, useRoute } from "./shared/route";
import { useTheme } from "./shared/use-theme";

export function App() {
  const [route, navigate] = useRoute();
  const [theme, toggleTheme] = useTheme();
  const [toast, setToast] = useState<string | null>(null);
  const [shown, setShown] = useState(0);
  const libraries = useLibraries();
  const onScanFailed = useCallback((error: IpcFailure) => setToast(describeError(error)), []);
  const scans = useScanStatus(onScanFailed);
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
          <button className="tbtn" onClick={toggleTheme} aria-label={theme === "dark" ? "Switch to light theme" : "Switch to dark theme"} title="Light / dark theme">
            <Icon name={theme === "dark" ? "sun" : "moon"} />
          </button>
          <button className="tbtn" onClick={() => library && void rescan(library)} disabled={!library || isScanning(scans[library.id])} aria-label="Scan for changes" title="Scan for changes">
            <Icon name="refresh" />
          </button>
          <button className="tbtn" onClick={() => navigate({ view: "settings" })} aria-label="Settings" title="Settings">
            <Icon name="settings" />
          </button>
        </div>
      </header>

      <aside className="side" aria-label="Sidebar">
        <div className="side-scroll">
          {list.map((item) => (
            <div className="sec" key={item.id}>
              <h3>{item.name}</h3>
              <button className={`nav${libraryRoute && item.id === library?.id && !libraryRoute.folder && !libraryRoute.all ? " on" : ""}`} onClick={() => navigate({ ...HOME, library: item.id })}>
                <Icon name="folder" />Browse folders
              </button>
              <button className={`nav${libraryRoute && item.id === library?.id && !libraryRoute.folder && libraryRoute.all ? " on" : ""}`} onClick={() => navigate({ ...HOME, library: item.id, all: true })}>
                <Icon name="grid" />All models
              </button>
              <FolderTree
                library={item.id}
                current={libraryRoute && item.id === library?.id ? libraryRoute.folder : null}
                onOpen={(folder) => navigate({ ...HOME, library: item.id, folder })}
                onMenu={(folder) => menu.open(item.id, folder.path, folder.isCollection)}
              />
            </div>
          ))}
        </div>
        {library && <SourceCard library={library} scan={scans[library.id]} />}
      </aside>

      <main className="main">
        {libraries.isError ? (
          <div className="empty err">{describeError(libraries.error)}</div>
        ) : libraries.isPending ? null : route.view === "settings" ? (
          <SettingsView libraries={list} scans={scans} onError={setToast} />
        ) : route.view === "model" ? (
          <ModelPage id={route.id} libraries={list} onBack={() => history.back()} onOpenFolder={(id, folder) => navigate({ ...HOME, library: id, folder, all: true })} />
        ) : library && libraryRoute ? (
          <LibraryView
            key={library.id}
            library={library}
            route={{ ...libraryRoute, library: library.id }}
            navigate={navigate}
            onFolderMenu={(folder) => menu.open(library.id, folder.path, folder.isCollection)}
            onShownChange={setShown}
          />
        ) : (
          <div className="empty">
            <h2>Add your first library</h2>
            <span>Pick the folder with your STL and 3MF files. It stays untouched: 3D Hoard Deck only reads it.</span>
            <button className="btn primary" onClick={() => addLibrary.mutate()} disabled={addLibrary.isPending}>
              <Icon name="plus" />Add library folder
            </button>
          </div>
        )}
      </main>

      <footer className="status">
        {route.view === "library" && <span><b>{shown.toLocaleString()}</b> shown</span>}
        <span>{list.length === 1 ? "1 library" : `${list.length} libraries`}</span>
        <span className="spacer" />
        <span>Libraries are read-only</span>
      </footer>

      {menu.target && <FolderMenu target={menu.target} onClose={menu.close} onError={setToast} />}
      {toast && <div className="toast" role="status">{toast}</div>}
    </div>
  );
}

function SourceCard({ library, scan }: { library: LibraryDto; scan: ScanStatus | undefined }) {
  const scanning = isScanning(scan);
  const label = scan?.phase === "failed" ? "Scan failed" : scanning ? (scan?.phase === "grouping" ? "Grouping models…" : "Scanning…") : "Up to date";
  return (
    <div className="source">
      <div className="row">
        <Icon name="drive" />
        <b style={{ fontWeight: 500 }}>{library.name}</b>
        {library.readOnly && <span className="pill lock" style={{ marginLeft: "auto" }}>Read-only</span>}
      </div>
      <div className="path">{library.root}</div>
      {scanning && <div className="bar"><i /></div>}
      <div className="scanline">
        <span className={scan?.phase === "failed" ? "err" : undefined}>{label}</span>
        {scanning && scan && <span className="mono">{scan.files.toLocaleString()} files</span>}
      </div>
    </div>
  );
}

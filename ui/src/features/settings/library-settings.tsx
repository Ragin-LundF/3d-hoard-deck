import { useMutation, useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap, type AccessModeDto, type LibraryDto, type MetadataStorageDto } from "../../ipc";
import { Icon } from "../../shared/icons";
import { isScanning, type ScanStatus } from "../library/use-scan-status";
import { LibraryRules } from "./library-rules";

const ACCESS: { value: AccessModeDto; title: string; text: string }[] = [
  { value: "readOnly", title: "Never change files in this library", text: "Names, order, tags and folder roles live in the index only." },
  { value: "metadataWrite", title: "Write metadata files only", text: "Allows a .3dhoarddeck file in each model folder. Model files are never touched." },
  { value: "managed", title: "Metadata files, rename and move", text: "Reserved for renaming and moving files on disk, which comes in a later version; until then it behaves like metadata files only." },
];

type Props = { library: LibraryDto; scan: ScanStatus | undefined; onError: (message: string) => void };

export function LibrarySettings({ library, scan, onError }: Props) {
  const queryClient = useQueryClient();
  const save = useMutation({
    mutationFn: (settings: { access: AccessModeDto; metadata: MetadataStorageDto }) =>
      unwrap(commands.updateLibrarySettings(library.id, settings.access, settings.metadata)),
    onSuccess: (written) => {
      void queryClient.invalidateQueries({ queryKey: ["libraries"] });
      if (written > 0) onError(`Wrote ${written} metadata ${written === 1 ? "file" : "files"} for models that already had names, tags, marks or notes.`);
    },
    onError: (error) => onError(describeError(error)),
  });
  const rebuild = useMutation({
    mutationFn: () => unwrap(commands.rebuildFromSidecars(library.id)),
    onSuccess: (report) => {
      void queryClient.invalidateQueries();
      onError(report.restored === 0 && report.unreadable === 0
        ? "No metadata files were found in this library."
        : `Restored ${report.restored} models${report.unreadable > 0 ? `; ${report.unreadable} files could not be read` : ""}.`);
    },
    onError: (error) => onError(describeError(error)),
  });
  const importOrynt3d = useMutation({
    mutationFn: () => unwrap(commands.importOrynt3d(library.id)),
    onSuccess: (report) => {
      void queryClient.invalidateQueries();
      onError(report.updated === 0 && report.unreadable === 0
        ? "Nothing new to import from Orynt3D."
        : `Imported Orynt3D data for ${report.updated} models${report.unreadable > 0 ? `; ${report.unreadable} files could not be read` : ""}.`);
    },
    onError: (error) => onError(describeError(error)),
  });
  const rescan = (full: boolean) => unwrap(commands.scanLibrary(library.id, full)).catch((error) => onError(describeError(error)));
  const scanning = isScanning(scan);
  // Metadata files need write access; going back to read-only also switches them off.
  const setAccess = (access: AccessModeDto) =>
    save.mutate({ access, metadata: access === "readOnly" ? "indexOnly" : library.metadata });
  const name = `library-${library.id}`;

  return (
    <div className="panel">
      <div className="prow2">
        <div className="t">
          <b>{library.name}</b>
          <span className="mono">{library.root}</span>
        </div>
        <button className="btn" disabled={scanning} onClick={() => void rescan(false)}>
          <Icon name="refresh" />Rescan
        </button>
        <button className="btn" disabled={scanning} onClick={() => void rescan(true)} title="Also re-reads folders that look unchanged, to catch files overwritten in place">
          Full rescan
        </button>
      </div>
      <div className="prow2 stack">
        <div className="t"><b>What 3D Hoard Deck may change</b></div>
        <div className="radios" role="radiogroup" aria-label="Write access">
          {ACCESS.map((option) => (
            <label className="radio" key={option.value}>
              <input type="radio" name={`${name}-access`} checked={library.access === option.value} disabled={save.isPending} onChange={() => setAccess(option.value)} />
              <div><b>{option.title}</b><span>{option.text}</span></div>
            </label>
          ))}
        </div>
      </div>
      <div className="prow2 stack">
        <div className="t"><b>Where tags, notes and names are saved</b></div>
        <div className="radios" role="radiogroup" aria-label="Metadata storage">
          <label className="radio">
            <input type="radio" name={`${name}-meta`} checked={library.metadata === "indexOnly"} disabled={save.isPending} onChange={() => save.mutate({ access: library.access, metadata: "indexOnly" })} />
            <div><b>Only in the index</b><span>Nothing is written into the library folders. Back up the index folder to keep your work.</span></div>
          </label>
          <label className="radio">
            <input
              type="radio"
              name={`${name}-meta`}
              checked={library.metadata === "indexAndSidecar"}
              disabled={save.isPending || library.access === "readOnly"}
              onChange={() => save.mutate({ access: library.access, metadata: "indexAndSidecar" })}
            />
            <div>
              <b>Index and a <span className="code">.3dhoarddeck</span> file beside each model</b>
              <span>The whole index can then be rebuilt from the folders, on any computer. The file is updated whenever you change a model's name, tags, marks or notes.</span>
            </div>
          </label>
        </div>
        {library.access === "readOnly" && (
          <div className="callout">
            <Icon name="database" />
            <span>Metadata files need write access: choose "Write metadata files only" above first.</span>
          </div>
        )}
      </div>
      <div className="prow2">
        <div className="t">
          <b>Rebuild from metadata files</b>
          <span>Reads the <span className="code">.3dhoarddeck</span> files in this library. Where a file is newer than the values in the index, it replaces them: names, tags, marks and notes. Useful after moving to a new computer. Only reads the folders.</span>
        </div>
        <button className="btn" disabled={rebuild.isPending} onClick={() => rebuild.mutate()}>Rebuild</button>
      </div>
      <div className="prow2">
        <div className="t">
          <b>Import from Orynt3D</b>
          <span>
            Takes names, notes and tags from Orynt3D's <span className="code">config.orynt3d</span> files. Only reads them; your own entries here are kept, and running it again adds nothing twice.
            {library.metadata === "indexAndSidecar" && library.access !== "readOnly" && " Metadata files are on, so a .3dhoarddeck file is written for every model that gets data."}
          </span>
        </div>
        <button className="btn" disabled={importOrynt3d.isPending} onClick={() => importOrynt3d.mutate()}>Import</button>
      </div>
      <LibraryRules library={library.id} onError={onError} />
    </div>
  );
}

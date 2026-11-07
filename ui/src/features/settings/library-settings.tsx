import { useMutation, useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap, type AccessModeDto, type LibraryDto, type MetadataStorageDto } from "../../ipc";
import { Icon } from "../../shared/icons";
import { isScanning, type ScanStatus } from "../library/use-scan-status";
import { LibraryRules } from "./library-rules";
import { t, plural } from "../../shared/i18n";

const ACCESS: { value: AccessModeDto; title: string; text: string }[] = [
  { value: "readOnly", title: t("Never change files in this library"), text: t("Names, order, tags and folder roles live in the index only.") },
  { value: "metadataWrite", title: t("Write metadata files only"), text: t("Allows a .3dhoarddeck file in each model folder. Model files are never touched.") },
  { value: "managed", title: t("Metadata files, rename and move"), text: t("Also allows renaming models and moving them to other folders on disk. Every change can be undone.") },
];

/** Appended to a report: how many files could not be read, if any. */
const unreadable = (count: number) => (count > 0 ? " " + t("{count} files could not be read.", { count }) : "");

type Props = { library: LibraryDto; scan: ScanStatus | undefined; onError: (message: string) => void };

export function LibrarySettings({ library, scan, onError }: Props) {
  const queryClient = useQueryClient();
  const save = useMutation({
    mutationFn: (settings: { access: AccessModeDto; metadata: MetadataStorageDto }) =>
      unwrap(commands.updateLibrarySettings(library.id, settings.access, settings.metadata)),
    onSuccess: (written) => {
      void queryClient.invalidateQueries({ queryKey: ["libraries"] });
      if (written > 0) onError(plural(written, "Wrote {count} metadata file for models that already had names, tags, marks or notes.", "Wrote {count} metadata files for models that already had names, tags, marks or notes."));
    },
    onError: (error) => onError(describeError(error)),
  });
  const rebuild = useMutation({
    mutationFn: () => unwrap(commands.rebuildFromSidecars(library.id)),
    onSuccess: (report) => {
      void queryClient.invalidateQueries();
      onError(report.restored === 0 && report.unreadable === 0
        ? t("No metadata files were found in this library.")
        : t("Restored {count} models.", { count: report.restored }) + unreadable(report.unreadable));
    },
    onError: (error) => onError(describeError(error)),
  });
  const importOrynt3d = useMutation({
    mutationFn: () => unwrap(commands.importOrynt3d(library.id)),
    onSuccess: (report) => {
      void queryClient.invalidateQueries();
      onError(report.updated === 0 && report.unreadable === 0
        ? t("Nothing new to import from Orynt3D.")
        : t("Imported Orynt3D data for {count} models.", { count: report.updated }) + unreadable(report.unreadable));
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
          <Icon name="refresh" />{t("Rescan")}
        </button>
        <button className="btn" disabled={scanning} onClick={() => void rescan(true)} title={t("Also re-reads folders that look unchanged, to catch files overwritten in place")}>
          {t("Full rescan")}
        </button>
      </div>
      {!scanning && scan && scan.unreadable > 0 && (
        <div className="prow2 stack">
          <div className="t">
            <b className="err">{plural(scan.unreadable, "{count} folder or file could not be read", "{count} folders or files could not be read")}</b>
            <span>{t("Their earlier index data is kept, and the next scan tries again. Each line names the path and the reason the system gave.")}</span>
          </div>
          <pre className="failures">{scan.unreadableExamples.join("\n")}</pre>
          <div className="actions">
            <button className="btn" onClick={() => void navigator.clipboard.writeText(scan.unreadableExamples.join("\n"))}>{t("Copy list")}</button>
          </div>
        </div>
      )}
      <div className="prow2 stack">
        <div className="t"><b>{t("What 3D Hoard Deck may change")}</b></div>
        <div className="radios" role="radiogroup" aria-label={t("Write access")}>
          {ACCESS.map((option) => (
            <label className="radio" key={option.value}>
              <input type="radio" name={`${name}-access`} checked={library.access === option.value} disabled={save.isPending} onChange={() => setAccess(option.value)} />
              <div><b>{option.title}</b><span>{option.text}</span></div>
            </label>
          ))}
        </div>
      </div>
      <div className="prow2 stack">
        <div className="t"><b>{t("Where tags, notes and names are saved")}</b></div>
        <div className="radios" role="radiogroup" aria-label={t("Metadata storage")}>
          <label className="radio">
            <input type="radio" name={`${name}-meta`} checked={library.metadata === "indexOnly"} disabled={save.isPending} onChange={() => save.mutate({ access: library.access, metadata: "indexOnly" })} />
            <div><b>{t("Only in the index")}</b><span>{t("Nothing is written into the library folders. Back up the index folder to keep your work.")}</span></div>
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
              <b>{t("Index and a .3dhoarddeck file beside each model")}</b>
              <span>{t("The whole index can then be rebuilt from the folders, on any computer. The file is updated whenever you change a model's name, tags, marks or notes.")}</span>
            </div>
          </label>
        </div>
        {library.access === "readOnly" && (
          <div className="callout">
            <Icon name="database" />
            <span>{t("Metadata files need write access: choose “Write metadata files only” above first.")}</span>
          </div>
        )}
      </div>
      <div className="prow2">
        <div className="t">
          <b>{t("Rebuild from metadata files")}</b>
          <span>{t("Reads the .3dhoarddeck files in this library. Where a file is newer than the values in the index, it replaces them: names, tags, marks and notes. Useful after moving to a new computer. Only reads the folders.")}</span>
        </div>
        <button className="btn" disabled={rebuild.isPending} onClick={() => rebuild.mutate()}>{t("Rebuild")}</button>
      </div>
      <div className="prow2">
        <div className="t">
          <b>{t("Import from Orynt3D")}</b>
          <span>
            {t("Takes names, notes and tags from Orynt3D's config.orynt3d files. Only reads them; your own entries here are kept, and running it again adds nothing twice.")}
            {library.metadata === "indexAndSidecar" && library.access !== "readOnly" && " " + t("Metadata files are on, so a .3dhoarddeck file is written for every model that gets data.")}
          </span>
        </div>
        <button className="btn" disabled={importOrynt3d.isPending} onClick={() => importOrynt3d.mutate()}>{t("Import")}</button>
      </div>
      <LibraryRules library={library.id} onError={onError} />
    </div>
  );
}

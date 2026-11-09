import { useQuery } from "@tanstack/react-query";
import { commands, unwrap, type LibraryDto, type ModelFileDto_Serialize } from "../../ipc";
import { fileName, folderName, formatBytes } from "../../shared/format";
import { plural, t } from "../../shared/i18n";
import { settingsRoute, useRoute } from "../../shared/route";
import { openExtract } from "./extract-store";

/** `.part2.rar` and later, or `.r00` volumes: they belong to the first part. */
export function isLaterPart(path: string): boolean {
  const name = fileName(path).toLowerCase();
  const part = /\.part(\d+)\.rar$/.exec(name);
  if (part) return Number(part[1]) > 1;
  return /\.r\d\d$/.test(name);
}

export const isArchive = (path: string) => /\.(zip|rar|7z)$/i.test(path);

type Props = { model: number; library: LibraryDto; files: ModelFileDto_Serialize[] };

/** On a model page with archives: extract them, or delete an archive that was extracted before. */
export function ArchiveActions({ model, library, files }: Props) {
  const [, navigate] = useRoute();
  const archives = files.filter((file) => file.kind === "archive" && !isLaterPart(file.path));
  const pending = useQuery({ queryKey: ["pending-extractions", model], queryFn: () => unwrap(commands.pendingExtractions(model)), enabled: archives.length > 0 });
  if (archives.length === 0) return null;
  const managed = library.access === "managed";
  const size = archives.reduce((sum, file) => sum + (file.size ?? 0), 0);
  const items = archives.map((file) => ({ model, archive: file.path }));

  return (
    <div className="xcallout">
      <div className="xcallout-row">
        <span>
          <b>{plural(archives.length, "This model holds an archive", "This model holds {count} archives")}</b>
          <span className="muted"> · {formatBytes(size)}</span>
        </span>
        <button className="btn" disabled={!managed} onClick={() => openExtract({ library, items })}>
          {archives.length === 1 ? t("Extract…") : t("Extract all {count}…", { count: archives.length })}
        </button>
      </div>
      {archives.length > 1 && archives.map((file) => (
        <div className="xcallout-row sub" key={file.path}>
          <span className="muted">{fileName(file.path)} · {formatBytes(file.size ?? 0)}</span>
          <button className="btn ghost" disabled={!managed} onClick={() => openExtract({ library, items: [{ model, archive: file.path }] })}>{t("Extract…")}</button>
        </div>
      ))}
      {!managed && (
        <p className="hint">
          {t("Allow file changes (“rename and move”) for this library to extract archives.")}{" "}
          <button className="linkish" onClick={() => navigate(settingsRoute("library", library.id, "access"))}>{t("Open library settings")}</button>
        </p>
      )}
      {(pending.data ?? []).map((entry) => (
        <div className="xcallout-row" key={entry.journal}>
          <span className="muted">{t("{archive} was extracted to “{folder}”.", { archive: fileName(entry.archive), folder: folderName(entry.folder) })}</span>
          <button className="btn ghost" disabled={!managed} onClick={() => openExtract({ library, items: [{ model, archive: entry.archive }], pending: [entry] })}>{t("Delete archive…")}</button>
        </div>
      ))}
    </div>
  );
}

type FolderProps = { library: LibraryDto; archives: { id: number; path: string; size: number }[] };

/** Above the grid when archives lie here as models of their own: extract them all at once. */
export function FolderArchives({ library, archives }: FolderProps) {
  const [, navigate] = useRoute();
  const first = archives.filter((archive) => !isLaterPart(archive.path));
  if (first.length === 0) return null;
  const managed = library.access === "managed";
  const size = archives.reduce((sum, archive) => sum + archive.size, 0);
  return (
    <div className="xcallout banner">
      <div className="xcallout-row">
        <span>
          <b>{plural(first.length, "This folder holds an archive", "This folder holds {count} archives")}</b>
          <span className="muted"> · {formatBytes(size)} · {t("Extract them into folders so their models get previews, search and tags.")}</span>
        </span>
        <button className="btn primary" disabled={!managed} onClick={() => openExtract({ library, items: first.map((archive) => ({ model: archive.id, archive: archive.path })) })}>{t("Extract all…")}</button>
      </div>
      {!managed && (
        <p className="hint">
          {t("Allow file changes (“rename and move”) for this library to extract archives.")}{" "}
          <button className="linkish" onClick={() => navigate(settingsRoute("library", library.id, "access"))}>{t("Open library settings")}</button>
        </p>
      )}
    </div>
  );
}

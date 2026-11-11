import { useQuery } from "@tanstack/react-query";
import { commands, describeError, unwrap } from "../../ipc";
import { formatBytes } from "../../shared/format";
import type { MeshFormat } from "../viewer/formats";
import { t } from "../../shared/i18n";
import { openContextMenu } from "../../shared/context-menu";
import { copyPath, revealPath } from "../../shared/file-actions";

type Props = {
  library: number;
  archive: string;
  viewing: string | null;
  onView: (path: string, format: MeshFormat) => void;
  /** The library's folder on this machine; "Show in folder" shows the archive. */
  root: string | null;
  onMessage: (message: string) => void;
};

/** Matches the shell's limit for one archive listing. */
const MAX_ENTRIES = 5000;

/** STL and OBJ inside a ZIP open in the viewer; 3MF needs the file on disk, so it is only listed. */
function entryFormat(name: string): MeshFormat | null {
  const lower = name.toLowerCase();
  if (lower.endsWith(".stl") || lower.includes(".stl.")) return "stl";
  return lower.endsWith(".obj") ? "obj" : null;
}

/** The files inside a ZIP, read without unpacking it. */
export function ArchiveContents({ library, archive, viewing, onView, root, onMessage }: Props) {
  const contents = useQuery({
    queryKey: ["archive", library, archive],
    queryFn: () => unwrap(commands.archiveContents(library, archive)),
  });
  if (contents.isPending) return <div className="frow inner"><span /><span className="fn">{t("Reading archive…")}</span></div>;
  if (contents.isError) return <div className="frow inner err"><span /><span className="fn">{describeError(contents.error)}</span></div>;
  if (contents.data.length === 0) return <div className="frow inner"><span /><span className="fn">{t("The archive is empty.")}</span></div>;
  const capped = contents.data.length >= MAX_ENTRIES;
  return [...contents.data.map((entry) => {
    const format = entryFormat(entry.name);
    const path = `${archive}!/${entry.name}`;
    return (
      <div
        key={entry.name}
        className={["frow inner", format ? "viewable" : "", path === viewing ? "active" : ""].join(" ")}
        title={format ? `${entry.name}\n${t("Click to preview in 3D")}` : entry.name}
        onClick={() => format && onView(path, format)}
        onContextMenu={(event) => openContextMenu(event, [
          { label: t("Preview in 3D"), hidden: !format, onSelect: () => format && onView(path, format) },
          { label: t("Show the archive in its folder"), hidden: root === null, onSelect: () => root !== null && revealPath(root, archive, onMessage) },
          { label: t("Copy path"), hidden: root === null, onSelect: () => root !== null && copyPath(root, archive, onMessage) },
        ])}
      >
        <span />
        <span className="fn">{entry.name}</span>
        <span className="fs">{formatBytes(entry.size)}</span>
      </div>
    );
  }), capped && <div className="frow inner" key="capped"><span /><span className="fn">{t("Only the first {count} files are listed.", { count: MAX_ENTRIES })}</span></div>];
}

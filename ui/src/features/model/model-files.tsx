import { Fragment, useState } from "react";
import { commands, describeError, unwrap, type LibraryDto } from "../../ipc";
import { openContextMenu, type MenuItem } from "../../shared/context-menu";
import { copyPath, revealPath } from "../../shared/file-actions";
import { isArchive, isLaterPart } from "../files/archive-actions";
import { openExtract } from "../files/extract-store";
import { useSlicers } from "../slicers/queries";
import type { ModelFileDto_Serialize } from "../../ipc";
import { ArchiveContents } from "./archive-contents";
import { fileName, formatBytes, formatClass } from "../../shared/format";
import { viewerFormat, type MeshFormat } from "../viewer/formats";
import { t } from "../../shared/i18n";

type Props = {
  library: number;
  /** The model these files belong to, and its library: for "Open in", "Show in folder", "Copy path", "Extract…". */
  model: number;
  libraryInfo: LibraryDto | undefined;
  onMessage: (message: string) => void;
  files: ModelFileDto_Serialize[];
  viewing: string | null;
  onView: (path: string, format: MeshFormat) => void;
  onReveal: (path: string) => void;
};

function groupByVariant(files: ModelFileDto_Serialize[]): [string, ModelFileDto_Serialize[]][] {
  const groups = new Map<string, ModelFileDto_Serialize[]>();
  for (const file of files) {
    const key = file.variant ?? "";
    groups.set(key, [...(groups.get(key) ?? []), file]);
  }
  return [...groups.entries()];
}

const isZip = (path: string) => path.toLowerCase().endsWith(".zip");

/** Files grouped by variant folder. Click a mesh to preview it in 3D, a ZIP to see inside, double-click to reveal. */
export function ModelFiles({ library, files, viewing, onView, onReveal, model, libraryInfo, onMessage }: Props) {
  const [open, setOpen] = useState<string | null>(null);
  const slicers = useSlicers();
  const slicer = slicers.data?.[0];
  const root = libraryInfo?.root ?? null;
  /** The menu of a file row; files inside a ZIP get the archive's "Show in folder". */
  const fileMenu = (file: ModelFileDto_Serialize): MenuItem[] => {
    const format = viewerFormat(file.kind);
    const archive = isArchive(file.path);
    return [
      { label: t("Preview in 3D"), hidden: !format, onSelect: () => format && onView(file.path, format) },
      {
        label: t("Open in {name}", { name: slicer?.name ?? "" }),
        hidden: !slicer || !slicer.extensions.some((extension) => file.path.toLowerCase().endsWith(`.${extension.toLowerCase()}`)),
        onSelect: () => void unwrap(commands.openInSlicer(model, [file.path], null)).then((name) => onMessage(t("Opened in {name}", { name }))).catch((error) => onMessage(describeError(error))),
      },
      { label: t("Show contents"), hidden: !isZip(file.path), onSelect: () => setOpen(file.path) },
      {
        label: t("Extract…"),
        hidden: !archive || isLaterPart(file.path) || libraryInfo?.access !== "managed",
        onSelect: () => libraryInfo && openExtract({ library: libraryInfo, items: [{ model, archive: file.path }] }),
      },
      "separator",
      { label: t("Show in folder"), hidden: root === null, onSelect: () => root !== null && revealPath(root, file.path, onMessage) },
      { label: t("Copy path"), hidden: root === null, onSelect: () => root !== null && copyPath(root, file.path, onMessage) },
    ];
  };
  return groupByVariant(files).map(([variant, group]) => (
    <div className="fgroup" key={variant}>
      <h4>{variant || t("Main folder")}</h4>
      {group.map((file) => {
        const format = viewerFormat(file.kind);
        const zip = isZip(file.path);
        const classes = ["frow", format || zip ? "viewable" : "", file.path === viewing ? "active" : ""].join(" ");
        const hint = format ? t("Click to preview in 3D") : zip ? t("Click to see what is inside") : null;
        return (
          <Fragment key={file.path}>
          <div
            className={classes}
            title={hint ? `${file.path}\n${hint}. ${t("Double-click to show in folder.")}` : file.path}
            aria-expanded={zip ? open === file.path : undefined}
            onClick={() => (format ? onView(file.path, format) : zip && setOpen(open === file.path ? null : file.path))}
            onDoubleClick={() => onReveal(file.path)}
            onContextMenu={(event) => openContextMenu(event, fileMenu(file))}
          >
            <span className={formatClass(file.kind)}>{file.kind}</span>
            <span className="fn">{fileName(file.path)}</span>
            <span className="fs">{formatBytes(file.size)}</span>
          </div>
          {open === file.path && <ArchiveContents library={library} archive={file.path} viewing={viewing} onView={onView} root={root} onMessage={onMessage} />}
          </Fragment>
        );
      })}
    </div>
  ));
}

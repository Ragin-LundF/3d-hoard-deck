import { Fragment, useState } from "react";
import type { ModelFileDto_Serialize } from "../../ipc";
import { ArchiveContents } from "./archive-contents";
import { fileName, formatBytes, formatClass } from "../../shared/format";
import { viewerFormat, type MeshFormat } from "../viewer/formats";
import { t } from "../../shared/i18n";

type Props = {
  library: number;
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
export function ModelFiles({ library, files, viewing, onView, onReveal }: Props) {
  const [open, setOpen] = useState<string | null>(null);
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
          >
            <span className={formatClass(file.kind)}>{file.kind}</span>
            <span className="fn">{fileName(file.path)}</span>
            <span className="fs">{formatBytes(file.size)}</span>
          </div>
          {open === file.path && <ArchiveContents library={library} archive={file.path} viewing={viewing} onView={onView} />}
          </Fragment>
        );
      })}
    </div>
  ));
}

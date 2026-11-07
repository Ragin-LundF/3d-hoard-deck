import type { ModelFileDto_Serialize } from "../../ipc";
import { fileName, formatBytes, formatClass } from "../../shared/format";
import { viewerFormat, type MeshFormat } from "../viewer/formats";

type Props = {
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

/** Files grouped by variant folder. Click a mesh to preview it in 3D, double-click any file to reveal it. */
export function ModelFiles({ files, viewing, onView, onReveal }: Props) {
  return groupByVariant(files).map(([variant, group]) => (
    <div className="fgroup" key={variant}>
      <h4>{variant || "Main folder"}</h4>
      {group.map((file) => {
        const format = viewerFormat(file.kind);
        const classes = ["frow", format ? "viewable" : "", file.path === viewing ? "active" : ""].join(" ");
        return (
          <div
            className={classes}
            key={file.path}
            title={format ? `${file.path}\nClick to preview in 3D, double-click to show in folder` : file.path}
            onClick={() => format && onView(file.path, format)}
            onDoubleClick={() => onReveal(file.path)}
          >
            <span className={formatClass(file.kind)}>{file.kind}</span>
            <span className="fn">{fileName(file.path)}</span>
            <span className="fs">{formatBytes(file.size)}</span>
          </div>
        );
      })}
    </div>
  ));
}

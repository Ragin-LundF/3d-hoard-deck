import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { useState } from "react";
import { commands, describeError, fileUrl, unwrap, type LibraryDto, type ModelFileDto_Serialize } from "../../ipc";
import { fileName, formatBytes, formatClass, folderName } from "../../shared/format";
import { Icon } from "../../shared/icons";

type Props = { id: number; libraries: LibraryDto[]; onBack: () => void; onOpenFolder: (library: number, folder: string) => void };

/** Absolute path on this machine, in the root's own separator style. */
function absolutePath(root: string, path: string): string {
  const separator = root.includes("\\") ? "\\" : "/";
  return `${root.replace(/[\\/]+$/, "")}${separator}${path.split("/").join(separator)}`;
}

function groupByVariant(files: ModelFileDto_Serialize[]): [string, ModelFileDto_Serialize[]][] {
  const groups = new Map<string, ModelFileDto_Serialize[]>();
  for (const file of files) {
    const key = file.variant ?? "";
    groups.set(key, [...(groups.get(key) ?? []), file]);
  }
  return [...groups.entries()];
}

export function ModelPage({ id, libraries, onBack, onOpenFolder }: Props) {
  const { data, error, isPending } = useQuery({ queryKey: ["model", id], queryFn: () => unwrap(commands.getModel(id)) });
  const [picked, setPicked] = useState<string | null>(null);
  const [tab, setTab] = useState<"files" | "details">("files");
  const queryClient = useQueryClient();
  const detectAutomatically = useMutation({
    mutationFn: (model: { library: number; path: string }) => unwrap(commands.setFolderRole(model.library, model.path, null)),
    onSuccess: async () => {
      await queryClient.invalidateQueries();
      onBack();
    },
  });
  if (isPending) return <div className="empty">Loading…</div>;
  if (error) return <div className="empty err">{describeError(error)}</div>;
  if (!data) return <div className="empty">This model is no longer in the library.</div>;

  const { summary, files } = data;
  const library = libraries.find((candidate) => candidate.id === summary.library);
  const images = files.filter((file) => file.kind === "image");
  const shown = picked ?? summary.cover ?? images[0]?.path ?? null;
  const reveal = (path: string) => library && void revealItemInDir(absolutePath(library.root, path));

  return (
    <>
      <div className="dhead">
        <button className="back" onClick={onBack}><Icon name="back" />Back</button>
        <span className="crumbs">{summary.folder}</span>
      </div>
      <div className="dbody">
        <div className="stagecol">
          <div className="stage">
            {shown ? <img src={fileUrl(summary.library, shown)} alt={summary.title} /> : <span className="crumbs">No preview image in this folder</span>}
          </div>
          {images.length > 1 && (
            <div className="gallery" aria-label="Images">
              {images.map((image) => (
                <button key={image.path} className={image.path === shown ? "on" : ""} onClick={() => setPicked(image.path)} title={fileName(image.path)}>
                  <img src={fileUrl(summary.library, image.path)} alt="" loading="lazy" />
                </button>
              ))}
            </div>
          )}
        </div>
        <div className="info">
          <div className="info-top">
            <h2>{summary.title}</h2>
            <div className="mpath">{summary.path}</div>
            <div className="actions">
              <button className="btn" onClick={() => reveal(summary.path)} disabled={!library}>
                <Icon name="folder" />Show in folder
              </button>
              {summary.markedByUser && (
                <button
                  className="btn"
                  title="You marked this folder as one model. Split it again the way it is detected automatically."
                  disabled={detectAutomatically.isPending}
                  onClick={() => detectAutomatically.mutate({ library: summary.library, path: summary.path })}
                >
                  Detect automatically
                </button>
              )}
              {summary.collection && (
                <button className="btn" onClick={() => onOpenFolder(summary.library, summary.collection ?? "")}>
                  <Icon name="box" />{folderName(summary.collection)}
                </button>
              )}
            </div>
            {detectAutomatically.isError && <span className="err">{describeError(detectAutomatically.error)}</span>}
          </div>
          <div className="itabs" role="tablist">
            <button role="tab" aria-selected={tab === "files"} className={tab === "files" ? "on" : ""} onClick={() => setTab("files")}>Files ({files.length})</button>
            <button role="tab" aria-selected={tab === "details"} className={tab === "details" ? "on" : ""} onClick={() => setTab("details")}>Details</button>
          </div>
          <div className="ipane" role="tabpanel">
            {tab === "files" ? (
              groupByVariant(files).map(([variant, group]) => (
                <div className="fgroup" key={variant}>
                  <h4>{variant || "Main folder"}</h4>
                  {group.map((file) => (
                    <div className="frow" key={file.path} onDoubleClick={() => reveal(file.path)} title={file.path}>
                      <span className={formatClass(file.kind)}>{file.kind}</span>
                      <span className="fn">{fileName(file.path)}</span>
                      <span className="fs">{formatBytes(file.size)}</span>
                    </div>
                  ))}
                </div>
              ))
            ) : (
              <dl className="kv">
                <dt>Formats</dt><dd>{summary.formats.join(", ").toUpperCase() || "–"}</dd>
                <dt>Files</dt><dd>{summary.fileCount}</dd>
                <dt>Size</dt><dd>{formatBytes(summary.totalSize)}</dd>
                <dt>Released</dt><dd>{summary.released ?? "–"}</dd>
                <dt>Collection</dt><dd>{summary.collection ?? "–"}</dd>
                <dt>Library</dt><dd>{library?.name ?? "–"}</dd>
              </dl>
            )}
          </div>
        </div>
      </div>
    </>
  );
}

import { useQuery } from "@tanstack/react-query";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { useState } from "react";
import { commands, describeError, unwrap, type LibraryDto, type ModelFileDto_Serialize } from "../../ipc";
import { Icon } from "../../shared/icons";
import { viewerFormat, type MeshFormat } from "../viewer/formats";
import { ModelDetails } from "./model-details";
import { ModelFiles } from "./model-files";
import { ModelHeader } from "./model-header";
import { ModelStage, type StageView } from "./model-stage";
import { useMetaSaver } from "../organize/use-save-meta";

type Props = {
  id: number;
  libraries: LibraryDto[];
  onBack: () => void;
  onOpenFolder: (library: number, folder: string) => void;
  onMessage: (message: string) => void;
};

/** Absolute path on this machine, in the root's own separator style. */
function absolutePath(root: string, path: string): string {
  const separator = root.includes("\\") ? "\\" : "/";
  return `${root.replace(/[\\/]+$/, "")}${separator}${path.split("/").join(separator)}`;
}

function firstViewable(files: ModelFileDto_Serialize[]): { path: string; format: MeshFormat } | null {
  for (const file of files) {
    const format = viewerFormat(file.kind);
    if (format) return { path: file.path, format };
  }
  return null;
}

export function ModelPage({ id, libraries, onBack, onOpenFolder, onMessage }: Props) {
  const { data, error, isPending } = useQuery({ queryKey: ["model", id], queryFn: () => unwrap(commands.getModel(id)) });
  const saveMetaFor = useMetaSaver(onMessage);
  const [picked, setPicked] = useState<StageView | null>(null);
  const [tab, setTab] = useState<"files" | "details">("files");
  if (isPending) return <div className="empty">Loading…</div>;
  if (error) return <div className="empty err">{describeError(error)}</div>;
  if (!data) return <div className="empty">This model is no longer in the library.</div>;

  const { summary, files } = data;
  const library = libraries.find((candidate) => candidate.id === summary.library);
  const images = files.filter((file) => file.kind === "image");
  const firstMesh = firstViewable(files);
  // Folder images first; models without any image open straight in 3D.
  const cover = summary.cover ?? images[0]?.path ?? null;
  const view: StageView = picked ?? (cover || !firstMesh ? { kind: "image", path: cover } : { kind: "mesh", ...firstMesh });
  const reveal = (path: string) => library && void revealItemInDir(absolutePath(library.root, path));
  const viewing = view.kind === "mesh" ? view.path : null;
  const saveMeta = (change: Partial<typeof data.meta>) => saveMetaFor(data, change);

  return (
    <>
      <div className="dhead">
        <button className="back" onClick={onBack}><Icon name="back" />Back</button>
        <span className="crumbs">{summary.folder}</span>
      </div>
      <div className="dbody">
        <ModelStage
          library={summary.library}
          title={summary.title}
          view={view}
          images={images}
          firstMesh={firstMesh}
          onView={setPicked}
          cover={summary.cover}
          onUseCover={(path) => saveMeta({ cover: path })}
        />
        <div className="info">
          <ModelHeader
            model={data}
            viewing={viewing}
            canReveal={library !== undefined}
            onReveal={() => reveal(summary.path)}
            onBack={onBack}
            onOpenFolder={onOpenFolder}
            onMessage={onMessage}
          />
          <div className="itabs" role="tablist">
            <button role="tab" aria-selected={tab === "files"} className={tab === "files" ? "on" : ""} onClick={() => setTab("files")}>Files ({files.length})</button>
            <button role="tab" aria-selected={tab === "details"} className={tab === "details" ? "on" : ""} onClick={() => setTab("details")}>
              Tags &amp; notes{data.meta.tags.length > 0 ? ` (${data.meta.tags.length})` : ""}
            </button>
          </div>
          <div className="ipane" role="tabpanel">
            {tab === "files" ? (
              <ModelFiles files={files} viewing={viewing} onView={(path, format) => setPicked({ kind: "mesh", path, format })} onReveal={reveal} />
            ) : (
              <ModelDetails model={data} library={library} onMessage={onMessage} />
            )}
          </div>
        </div>
      </div>
    </>
  );
}

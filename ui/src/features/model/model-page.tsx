import { useQuery, useQueryClient } from "@tanstack/react-query";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { useState } from "react";
import { commands, describeError, unwrap, type LibraryDto, type ModelFileDto_Serialize } from "../../ipc";
import { folderName } from "../../shared/format";
import { Icon } from "../../shared/icons";
import { viewerFormat, type MeshFormat } from "../viewer/formats";
import { ModelDetails } from "./model-details";
import { ModelFiles } from "./model-files";
import { ModelHeader } from "./model-header";
import { ModelStage, type StageView } from "./model-stage";
import { useMetaSaver } from "../organize/use-save-meta";
import { t } from "../../shared/i18n";

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

/** `a/b/c` → `["a/b/c", "a/b", "a"]`: the folders a picture of this model can stand for. */
function parentFolders(folder: string): string[] {
  const parts = folder.split("/").filter(Boolean);
  return parts.map((_, index) => parts.slice(0, parts.length - index).join("/"));
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
  const queryClient = useQueryClient();
  const [picked, setPicked] = useState<StageView | null>(null);
  const [tab, setTab] = useState<"files" | "details">("files");
  if (isPending) return <div className="empty">{t("Loading…")}</div>;
  if (error) return <div className="empty err">{describeError(error)}</div>;
  if (!data) return <div className="empty">{t("This model is no longer in the library.")}</div>;

  const { summary, files } = data;
  const library = libraries.find((candidate) => candidate.id === summary.library);
  const images = files.filter((file) => file.kind === "image");
  const firstMesh = firstViewable(files);
  // Folder images first; models without any image open straight in 3D.
  // A 3MF's preview is only for cards; here the model opens in 3D.
  const cover = (summary.coverSource === "embedded" ? null : summary.cover) ?? images[0]?.path ?? null;
  const view: StageView = picked ?? (cover || !firstMesh ? { kind: "image", path: cover } : { kind: "mesh", ...firstMesh });
  const reveal = (path: string) => library && void revealItemInDir(absolutePath(library.root, path));
  const viewing = view.kind === "mesh" ? view.path : null;
  const saveMeta = (change: Partial<typeof data.meta>) => saveMetaFor(data, change);
  const useForFolder = async (folder: string, image: string) => {
    try {
      const copy = await unwrap(commands.setFolderCover(summary.library, folder, image));
      await queryClient.invalidateQueries();
      onMessage(copy ? t("Cover of {folder}, copied as {path}", { folder: folderName(folder), path: copy }) : t("Cover of {folder}", { folder: folderName(folder) }));
    } catch (error) {
      onMessage(describeError(error));
    }
  };
  // Saved next to the shown mesh as `preview_<name>.png`; only that folder is updated, and it becomes the cover by name.
  const saveScreenshot = async (png: Blob) => {
    if (view.kind !== "mesh") return;
    try {
      const bytes = Array.from(new Uint8Array(await png.arrayBuffer()));
      const shot = await unwrap(commands.saveScreenshot(summary.id, view.path, bytes));
      await queryClient.invalidateQueries();
      onMessage(shot.warning ?? t("Saved as {path}", { path: shot.path }));
    } catch (error) {
      onMessage(describeError(error));
    }
  };

  return (
    <>
      <div className="dhead">
        <button className="back" onClick={onBack}><Icon name="back" />{t("Back")}</button>
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
          onScreenshot={library && library.access !== "readOnly" ? saveScreenshot : null}
          folders={parentFolders(summary.folder)}
          onUseForFolder={(folder, image) => void useForFolder(folder, image)}
        />
        <div className="info">
          <ModelHeader
            model={data}
            viewing={viewing && /\.zip!\//i.test(viewing) ? null : viewing}
            canReveal={library !== undefined}
            canMove={library?.access === "managed"}
            onReveal={() => reveal(summary.path)}
            onBack={onBack}
            onOpenFolder={onOpenFolder}
            onMessage={onMessage}
          />
          <div className="itabs" role="tablist">
            <button role="tab" aria-selected={tab === "files"} className={tab === "files" ? "on" : ""} onClick={() => setTab("files")}>{t("Files")} ({files.length})</button>
            <button role="tab" aria-selected={tab === "details"} className={tab === "details" ? "on" : ""} onClick={() => setTab("details")}>
              {t("Tags & notes")}{data.meta.tags.length > 0 ? ` (${data.meta.tags.length})` : ""}
            </button>
          </div>
          <div className="ipane" role="tabpanel">
            {tab === "files" ? (
              <ModelFiles library={summary.library} files={files} viewing={viewing} onView={(path, format) => setPicked({ kind: "mesh", path, format })} onReveal={reveal} />
            ) : (
              <ModelDetails model={data} library={library} onMessage={onMessage} />
            )}
          </div>
        </div>
      </div>
    </>
  );
}

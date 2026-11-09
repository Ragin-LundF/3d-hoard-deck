import { lazy, Suspense, useEffect, useRef, useState, type ReactNode } from "react";
import { fileUrl, thumbUrl, type ModelFileDto_Serialize } from "../../ipc";
import { fileName, folderName } from "../../shared/format";
import type { MeshFormat } from "../viewer/formats";
import { t } from "../../shared/i18n";
import { Icon } from "../../shared/icons";
import { ZoomImage } from "./zoom-image";

// three.js is only downloaded when a model is opened in 3D.
const MeshViewer = lazy(() => import("../viewer/mesh-viewer"));

export type StageView = { kind: "image"; path: string | null } | { kind: "mesh"; path: string; format: MeshFormat };

type Props = {
  library: number;
  title: string;
  view: StageView;
  images: ModelFileDto_Serialize[];
  firstMesh: { path: string; format: MeshFormat } | null;
  onView: (view: StageView) => void;
  /** The cover shown in the grid; offered as an action when another image is shown. */
  cover: string | null;
  onUseCover: (path: string) => void;
  /** Saves a PNG of the 3D view into the model's folder; `null` when the library may not get files written. */
  onScreenshot: ((png: Blob) => Promise<void>) | null;
  /** Folders the shown image can be the card picture of, nearest first. */
  folders: string[];
  onUseForFolder: (folder: string, image: string) => void;
};

/** Left column of the model page: an image, or the interactive 3D preview of one mesh file. */
export function ModelStage({ library, title, view, images, firstMesh, onView, cover, onUseCover, onScreenshot, folders, onUseForFolder }: Props) {
  const firstImage = images[0]?.path ?? null;
  const [folderMenu, setFolderMenu] = useState(false);
  return (
    <div className="stagecol">
      <div className="stage">
        {view.kind === "mesh" ? (
          <Suspense fallback={<span className="crumbs">{t("Loading viewer…")}</span>}>
            <MeshViewer library={library} path={view.path} format={view.format} onScreenshot={onScreenshot} />
          </Suspense>
        ) : view.path ? (
          <ZoomImage src={fileUrl(library, view.path)} alt={title} />
        ) : (
          <span className="crumbs">{t("No preview image in this folder")}</span>
        )}
        {view.kind === "image" && view.path && (
          <div className="cover-actions">
            {view.path !== cover && <button className="btn" onClick={() => onUseCover(view.path ?? "")}>{t("Use as cover")}</button>}
            {folders.length > 0 && (
              <div className="split">
                <button className="btn" aria-haspopup="menu" aria-expanded={folderMenu} onClick={() => setFolderMenu(!folderMenu)}>
                  {t("Use for folder…")}
                </button>
                {folderMenu && (
                  <div className="menu folder-cover-menu" role="menu">
                    {folders.map((folder) => (
                      <button
                        key={folder}
                        role="menuitem"
                        title={folder}
                        onClick={() => {
                          setFolderMenu(false);
                          onUseForFolder(folder, view.path ?? "");
                        }}
                      >
                        {folderName(folder)}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>
        )}
        {firstMesh && (firstImage || view.kind === "image") && (
          <div className="vgroup stage-switch" role="group" aria-label={t("Preview")}>
            <button className={view.kind === "image" ? "on" : ""} aria-pressed={view.kind === "image"} onClick={() => onView({ kind: "image", path: firstImage })}>{t("Images")}</button>
            <button className={view.kind === "mesh" ? "on" : ""} aria-pressed={view.kind === "mesh"} onClick={() => onView({ kind: "mesh", ...firstMesh })}>3D</button>
          </div>
        )}
      </div>
      {images.length > 1 && (
        <Gallery>
          {images.map((image) => (
            <button key={image.path} className={view.kind === "image" && image.path === view.path ? "on" : ""} onClick={() => onView({ kind: "image", path: image.path })} title={fileName(image.path)}>
              <img src={thumbUrl(library, image.path)} alt="" loading="lazy" />
            </button>
          ))}
        </Gallery>
      )}
    </div>
  );
}

/** The image strip; arrows scroll it by a page for the mouse, shown only where there is more to see. */
function Gallery({ children }: { children: ReactNode }) {
  const strip = useRef<HTMLDivElement>(null);
  const [more, setMore] = useState({ left: false, right: false });
  useEffect(() => {
    const element = strip.current;
    if (!element) return;
    const update = () =>
      setMore({ left: element.scrollLeft > 1, right: element.scrollLeft + element.clientWidth < element.scrollWidth - 1 });
    update();
    const resized = new ResizeObserver(update);
    resized.observe(element);
    element.addEventListener("scroll", update, { passive: true });
    return () => {
      resized.disconnect();
      element.removeEventListener("scroll", update);
    };
  }, [children]);
  const page = (direction: number) => {
    const element = strip.current;
    element?.scrollBy({ left: direction * element.clientWidth * 0.8, behavior: "smooth" });
  };
  return (
    <div className="gallery-wrap">
      <div className="gallery" ref={strip} aria-label={t("Images")}>{children}</div>
      {more.left && <button className="gallery-arrow prev" onClick={() => page(-1)} aria-label={t("Previous images")}><Icon name="chevron" /></button>}
      {more.right && <button className="gallery-arrow next" onClick={() => page(1)} aria-label={t("More images")}><Icon name="chevron" /></button>}
    </div>
  );
}

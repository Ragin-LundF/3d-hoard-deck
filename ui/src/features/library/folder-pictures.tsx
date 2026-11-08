import { useEffect, useState } from "react";
import { fileUrl, thumbUrl } from "../../ipc";
import { fileName } from "../../shared/format";
import { Icon } from "../../shared/icons";
import { t } from "../../shared/i18n";
import { ZoomImage } from "../model/zoom-image";

/** Pictures lying in a folder that belong to no model, e.g. an overview next to model subfolders. */
export function FolderPictures({ library, images }: { library: number; images: string[] }) {
  const [shown, setShown] = useState<number | null>(null);
  useEffect(() => {
    if (shown === null) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setShown(null);
      if (event.key === "ArrowRight") setShown((index) => (index === null ? null : (index + 1) % images.length));
      if (event.key === "ArrowLeft") setShown((index) => (index === null ? null : (index - 1 + images.length) % images.length));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [shown, images.length]);
  if (images.length === 0) return null;
  const current = shown === null ? null : images[shown];

  return (
    <>
      <div className="folder-pictures" aria-label={t("Pictures in this folder")}>
        {images.map((image, index) => (
          <button key={image} onClick={() => setShown(index)} title={fileName(image)}>
            <img src={thumbUrl(library, image)} alt={fileName(image)} loading="lazy" />
          </button>
        ))}
      </div>
      {current && (
        <div className="lightbox" role="dialog" aria-modal="true" aria-label={fileName(current)}>
          <ZoomImage src={fileUrl(library, current)} alt={fileName(current)} />
          <div className="lightbox-bar">
            <span>{fileName(current)}</span>
            {images.length > 1 && (
              <>
                <button className="iconbtn" onClick={() => setShown(((shown ?? 0) - 1 + images.length) % images.length)} aria-label={t("Previous picture")}>◀</button>
                <button className="iconbtn" onClick={() => setShown(((shown ?? 0) + 1) % images.length)} aria-label={t("Next picture")}>▶</button>
              </>
            )}
            <button className="iconbtn" onClick={() => setShown(null)} aria-label={t("Close")} autoFocus><Icon name="close" /></button>
          </div>
        </div>
      )}
    </>
  );
}

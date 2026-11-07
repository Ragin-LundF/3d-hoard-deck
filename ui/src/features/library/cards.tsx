import { useState, type MouseEvent } from "react";
import { thumbUrl, type FolderDto_Serialize, type ModelSummaryDto_Serialize } from "../../ipc";
import { formatClass } from "../../shared/format";
import { Icon } from "../../shared/icons";

function Cover({ library, path, fallback }: { library: number; path: string | null; fallback: string }) {
  const [broken, setBroken] = useState(false);
  if (!path || broken) return <span className="ph">{fallback}</span>;
  return <img src={thumbUrl(library, path)} alt="" loading="lazy" decoding="async" onError={() => setBroken(true)} />;
}

export function ModelCard({ model, onOpen }: { model: ModelSummaryDto_Serialize; onOpen: () => void }) {
  return (
    <button className="card" onClick={onOpen} title={model.path}>
      <div className="cover">
        <Cover library={model.library} path={model.cover} fallback={(model.formats[0] ?? "file").toUpperCase()} />
        {model.fileCount > 1 && <span className="count">{model.fileCount} files</span>}
        {(model.favorite || model.printed) && (
          <span className="marks">
            {model.favorite && <span className="fav" title="Favorite"><Icon name="starFilled" /></span>}
            {model.printed && <span className="done" title="Printed"><Icon name="check" /></span>}
          </span>
        )}
      </div>
      <div className="cname">{model.title}</div>
      <div className="cmeta">
        {model.formats.map((format) => (
          <span key={format} className={formatClass(format)}>{format}</span>
        ))}
        {model.released && <span className="date">{model.released}</span>}
      </div>
    </button>
  );
}

type FolderCardProps = {
  library: number;
  folder: FolderDto_Serialize;
  onOpen: () => void;
  onMenu: (event: MouseEvent) => void;
};

export function FolderCard({ library, folder, onOpen, onMenu }: FolderCardProps) {
  return (
    <button className="card" onClick={onOpen} onContextMenu={onMenu} title={folder.path}>
      <div className="cover fcover">
        <div className="inner">
          <Cover library={library} path={folder.cover} fallback="" />
        </div>
        <span className={folder.isCollection ? "badge coll" : "badge"} style={{ zIndex: 2 }}>
          <Icon name={folder.isCollection ? "box" : "folder"} />
          {folder.isCollection ? "Collection" : "Folder"}
        </span>
        <span className="count" style={{ zIndex: 2 }}>{folder.totalModels.toLocaleString()}</span>
      </div>
      <div className="cname">{folder.name}</div>
      <div className="cmeta">{folder.released && <span className="date">{folder.released}</span>}</div>
    </button>
  );
}

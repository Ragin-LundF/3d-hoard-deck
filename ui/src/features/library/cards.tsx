import { useState, type MouseEvent } from "react";
import { thumbUrl, type FolderDto_Serialize, type ModelSummaryDto_Serialize } from "../../ipc";
import { formatClass } from "../../shared/format";
import { Icon } from "../../shared/icons";
import { t, plural, formatNumber } from "../../shared/i18n";
import { RenderedActions, RenderedCover } from "../render/rendered-cover";

type CoverProps = { library: number; path: string | null; fallback: string; onBroken?: () => void };

function Cover({ library, path, fallback, onBroken }: CoverProps) {
  const [broken, setBroken] = useState(false);
  if (!path || broken) return <span className="ph">{fallback}</span>;
  return <img src={thumbUrl(library, path)} alt="" loading="lazy" decoding="async" onError={() => (onBroken ? onBroken() : setBroken(true))} />;
}

type ModelCardProps = {
  model: ModelSummaryDto_Serialize;
  onOpen: () => void;
  selected?: boolean;
  /** While models are selected, a click selects instead of opening; ⌘/Ctrl- and ⇧-click always select. */
  selecting?: boolean;
  onSelect?: (event: MouseEvent) => void;
  /** The library may write files: a rendered preview is saved into the folder, else kept in the app. */
  canWrite?: boolean;
  onMessage?: (message: string) => void;
  /** Right-click (or the Context Menu key) on the card. */
  onMenu?: (event: MouseEvent) => void;
};

export function ModelCard({ model, onOpen, selected = false, selecting = false, onSelect, canWrite = false, onMessage = () => {}, onMenu }: ModelCardProps) {
  const click = (event: MouseEvent) => {
    if (onSelect && (selecting || event.metaKey || event.ctrlKey || event.shiftKey)) onSelect(event);
    else onOpen();
  };
  const waiting = !model.cover && model.rendered === "cached";
  // A 3MF without a preview inside shows the folder picture instead, if there is one.
  const [noEmbedded, setNoEmbedded] = useState(false);
  const fromFolder = noEmbedded && !!model.fallbackCover;
  const cover = noEmbedded ? model.fallbackCover : model.cover;
  const format = (model.formats[0] ?? "file").toUpperCase();
  return (
    <div className={`card-wrap${waiting ? " rendered" : ""}`}>
    {waiting && <RenderedActions model={model} canWrite={canWrite} onMessage={onMessage} />}
    <button className={`card${selected ? " sel" : ""}`} onClick={click} onContextMenu={onMenu} title={model.path} aria-pressed={selecting ? selected : undefined}>
      {onSelect && (
        <span className="cb" role="checkbox" aria-checked={selected} aria-label={t("Select {name}", { name: model.title })}
          onClick={(event) => { event.stopPropagation(); onSelect(event); }}>
          {selected && <Icon name="check" />}
        </span>
      )}
      <div className="cover">
        {model.cover ? (
          <Cover key={cover} library={model.library} path={cover} fallback={format}
            onBroken={model.coverSource === "embedded" && !noEmbedded ? () => setNoEmbedded(true) : undefined} />
        ) : (
          <RenderedCover model={model} fallback={format} />
        )}
        {(model.coverSource === "parent" || fromFolder) && <span className="borrowed" title={t("This model has no picture of its own; this one is from the folder above.")}>{t("from folder")}</span>}
        {model.fileCount > 1 && <span className="count">{plural(model.fileCount, "{count} file", "{count} files")}</span>}
        {(model.favorite || model.printed) && (
          <span className="marks">
            {model.favorite && <span className="fav" title={t("Favorite")}><Icon name="starFilled" /></span>}
            {model.printed && <span className="done" title={t("Printed")}><Icon name="check" /></span>}
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
    </div>
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
          {folder.isCollection ? t("Collection") : t("Folder")}
        </span>
        <span className="count" style={{ zIndex: 2 }}>{formatNumber(folder.totalModels)}</span>
      </div>
      <div className="cname">{folder.name}</div>
      <div className="cmeta">{folder.released && <span className="date">{folder.released}</span>}</div>
    </button>
  );
}

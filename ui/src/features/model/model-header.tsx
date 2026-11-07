import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { commands, describeError, unwrap, type ModelDetailDto_Serialize } from "../../ipc";
import { folderName } from "../../shared/format";
import { Icon } from "../../shared/icons";
import { useSaveMeta } from "../organize/use-save-meta";
import { OpenInSlicer } from "../slicers/open-in-slicer";
import { AddToCollection } from "../collections/add-to-collection";

type Props = {
  model: ModelDetailDto_Serialize;
  viewing: string | null;
  canReveal: boolean;
  onReveal: () => void;
  onBack: () => void;
  onOpenFolder: (library: number, folder: string) => void;
  onMessage: (message: string) => void;
};

/** Title (renamable, only in the app), marks and actions of the model page. */
export function ModelHeader({ model, viewing, canReveal, onReveal, onBack, onOpenFolder, onMessage }: Props) {
  const { summary, meta } = model;
  const save = useSaveMeta(model, onMessage);
  const [editing, setEditing] = useState(false);
  const queryClient = useQueryClient();
  const detectAutomatically = useMutation({
    mutationFn: () => unwrap(commands.setFolderRole(summary.library, summary.path, null)),
    onSuccess: async () => {
      await queryClient.invalidateQueries();
      onBack();
    },
  });
  const rename = (value: string) => {
    setEditing(false);
    const name = value.trim();
    save({ displayName: name && name !== model.originalTitle ? name : null });
  };

  return (
    <div className="info-top">
      <div className="mtitle">
        {editing ? (
          <input
            autoFocus
            defaultValue={summary.title}
            aria-label="Name"
            onBlur={(event) => rename(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") rename(event.currentTarget.value);
              if (event.key === "Escape") setEditing(false);
            }}
          />
        ) : (
          <h2>{summary.title}</h2>
        )}
        <button className="iconbtn" onClick={() => setEditing(true)} aria-label="Rename" title="Rename in 3D Hoard Deck (files stay as they are)">
          <Icon name="pen" />
        </button>
        <button className={`iconbtn${meta.favorite ? " fav" : ""}`} onClick={() => save({ favorite: !meta.favorite })} aria-pressed={meta.favorite} aria-label="Favorite" title="Favorite">
          <Icon name={meta.favorite ? "starFilled" : "star"} />
        </button>
      </div>
      {meta.displayName && (
        <div className="mpath">
          Folder name: {model.originalTitle} · <button className="linkish" onClick={() => save({ displayName: null })}>Use folder name</button>
        </div>
      )}
      <div className="mpath">{summary.path}</div>
      <div className="actions">
        <OpenInSlicer model={summary.id} file={viewing} onMessage={onMessage} />
        <button className="btn" onClick={onReveal} disabled={!canReveal}><Icon name="folder" />Show in folder</button>
        <AddToCollection model={summary.id} onMessage={onMessage} />
        <button className={`btn${meta.printed ? " on" : " ghost"}`} onClick={() => save({ printed: !meta.printed })} aria-pressed={meta.printed}>
          <Icon name="check" />{meta.printed ? "Printed" : "Mark printed"}
        </button>
        {summary.markedByUser && (
          <button className="btn" title="You marked this folder as one model. Split it again the way it is detected automatically." disabled={detectAutomatically.isPending} onClick={() => detectAutomatically.mutate()}>
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
  );
}

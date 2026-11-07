import { useMutation, useQuery } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { commands, describeError, IpcFailure, unwrap, type ExistingIndexDto } from "../../ipc";

type Conflict = { folder: string; modified: Date | null };

/** Where the index lives. Moving copies it (or switches to an index already there) and restarts the app at once. */
export function IndexSettings({ onError }: { onError: (message: string) => void }) {
  const location = useQuery({ queryKey: ["index-location"], queryFn: () => unwrap(commands.indexLocation()) });
  const [conflict, setConflict] = useState<Conflict | null>(null);
  const move = useMutation({
    mutationFn: ({ folder, existing }: { folder: string; existing: ExistingIndexDto }) =>
      unwrap(commands.moveIndexLocation(folder, existing)),
    onError: (error, { folder }) => {
      if (error instanceof IpcFailure && error.code === "index_exists") {
        const seconds = Number(error.details);
        setConflict({ folder, modified: Number.isFinite(seconds) ? new Date(seconds * 1000) : null });
      } else {
        onError(describeError(error));
      }
    },
  });
  const choose = async () => {
    const folder = await open({ directory: true, title: "Choose a folder for the index" });
    if (typeof folder === "string") move.mutate({ folder, existing: "ask" });
  };
  const decide = (existing: ExistingIndexDto) => {
    if (conflict) move.mutate({ folder: conflict.folder, existing });
    setConflict(null);
  };

  return (
    <div className="panel">
      <div className="prow2">
        <div className="t">
          <b>Index folder</b>
          <span className="mono">{location.data ?? "…"}</span>
          <span>Holds the database and the search index. Default is <span className="code">.3dhoarddeck</span> in your home folder. Pick a folder on a local disk: network shares and cloud-sync folders are refused because they damage the database.</span>
        </div>
        <button className="btn" onClick={() => void choose()} disabled={move.isPending || conflict !== null}>
          {move.isPending ? "Moving and restarting…" : "Choose folder…"}
        </button>
      </div>
      {conflict && (
        <div className="prow2 stack">
          <div className="t">
            <b>That folder already holds a 3D Hoard Deck index</b>
            <span className="mono">{conflict.folder}</span>
            <span>Last changed {conflict.modified ? conflict.modified.toLocaleString() : "at an unknown time"}. The app restarts right after you choose.</span>
          </div>
          <div className="actions">
            <button className="btn" onClick={() => decide("use")}>Use that index</button>
            <button className="btn" onClick={() => decide("replace")}>Replace it with my current index</button>
            <button className="btn ghost" onClick={() => setConflict(null)}>Cancel</button>
          </div>
        </div>
      )}
    </div>
  );
}

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { commands, describeError, unwrap, type LibraryDto } from "../../ipc";
import { Icon } from "../../shared/icons";
import { isScanning, type ScanStatus } from "../library/use-scan-status";

type Props = { libraries: LibraryDto[]; scans: Record<number, ScanStatus>; onError: (message: string) => void };

/** Asks for a folder, registers it read-only and starts the first scan. */
export function useAddLibrary(onError: (message: string) => void) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      const root = await open({ directory: true, title: "Choose a library folder" });
      if (typeof root !== "string") return null;
      const name = root.split(/[\\/]/).filter(Boolean).pop() ?? root;
      const library = await unwrap(commands.addLibrary(name, root));
      await unwrap(commands.scanLibrary(library.id, false));
      return library;
    },
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["libraries"] }),
    onError: (error) => onError(describeError(error)),
  });
}

export function SettingsView({ libraries, scans, onError }: Props) {
  const addLibrary = useAddLibrary(onError);
  const scan = (library: number, full: boolean) => unwrap(commands.scanLibrary(library, full)).catch((error) => onError(describeError(error)));
  return (
    <div className="setpane">
      <h1>Libraries</h1>
      <p className="lead">Folders 3D Hoard Deck indexes. Files stay where they are; the index only stores paths, metadata and covers. Libraries are read-only: nothing is written into them.</p>
      {libraries.map((library) => (
        <div className="panel" key={library.id}>
          <div className="prow2">
            <div className="t">
              <b>{library.name}</b>
              <span className="mono">{library.root}</span>
            </div>
            {library.readOnly && <span className="pill lock">Read-only</span>}
            <button className="btn" disabled={isScanning(scans[library.id])} onClick={() => void scan(library.id, false)}>
              <Icon name="refresh" />Rescan
            </button>
            <button className="btn" disabled={isScanning(scans[library.id])} onClick={() => void scan(library.id, true)} title="Also re-reads folders that look unchanged, to catch files overwritten in place">
              Full rescan
            </button>
          </div>
        </div>
      ))}
      <div>
        <button className="btn primary" onClick={() => addLibrary.mutate()} disabled={addLibrary.isPending}>
          <Icon name="plus" />Add library folder
        </button>
      </div>
    </div>
  );
}

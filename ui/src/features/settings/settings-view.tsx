import { useMutation, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { commands, describeError, unwrap, type LibraryDto } from "../../ipc";
import { Icon } from "../../shared/icons";
import type { ScanStatus } from "../library/use-scan-status";
import { FileJournal } from "../files/file-journal";
import { IndexSettings } from "./index-settings";
import { LibrarySettings } from "./library-settings";
import { SlicerSettings } from "./slicer-settings";
import { languageChoice, setLanguage, t, type LanguageChoice } from "../../shared/i18n";

type Props = { libraries: LibraryDto[]; scans: Record<number, ScanStatus>; onError: (message: string) => void };

/** Asks for a folder, registers it (nothing is written into it until the user allows it) and starts the first scan. */
export function useAddLibrary(onError: (message: string) => void) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      const root = await open({ directory: true, title: t("Choose a library folder") });
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
  return (
    <div className="setpane">
      <h1>{t("Settings")}</h1>
      <h2>{t("Libraries")}</h2>
      <p className="lead">{t("Folders 3D Hoard Deck indexes. Files stay where they are; each library decides what the app may change in it.")}</p>
      {libraries.map((library) => (
        <LibrarySettings key={library.id} library={library} scan={scans[library.id]} onError={onError} />
      ))}
      <div>
        <button className="btn primary" onClick={() => addLibrary.mutate()} disabled={addLibrary.isPending}>
          <Icon name="plus" />{t("Add library folder")}
        </button>
      </div>
      {libraries.some((library) => library.access === "managed") && (
        <>
          <h2>{t("Recent file changes")}</h2>
          <p className="lead">{t("Renames and moves on disk. Undo puts the files and their names, tags and notes back.")}</p>
          <FileJournal libraries={libraries} onError={onError} />
        </>
      )}
      <h2>{t("Slicers")}</h2>
      <p className="lead">{t("“Open in” uses the first slicer that opens the files; the others are in its menu.")}</p>
      <SlicerSettings onError={onError} />
      <h2>{t("Index")}</h2>
      <IndexSettings onError={onError} />
      <h2>{t("Language")}</h2>
      <div className="panel">
        <div className="prow2">
          <div className="t"><b>{t("Language of the app")}</b><span>{t("The app restarts its window to switch.")}</span></div>
          <select className="sel" aria-label={t("Language")} value={languageChoice()} onChange={(event) => setLanguage(event.target.value as LanguageChoice)}>
            <option value="system">{t("Like the system")}</option>
            <option value="en">English</option>
            <option value="de">Deutsch</option>
          </select>
        </div>
      </div>
    </div>
  );
}

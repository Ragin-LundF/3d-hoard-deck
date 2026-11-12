import { useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap, type FolderRoleDto, type LibraryDto } from "../../ipc";
import type { MenuItem } from "../../shared/context-menu";
import { copyPath, revealPath } from "../../shared/file-actions";
import { t } from "../../shared/i18n";
import { HOME, settingsRoute, useRoute } from "../../shared/route";

/** The menu of a folder, the same in the tree, as a card, in the breadcrumbs and behind its "⋯". */
export function useFolderMenu(onMessage: (message: string) => void) {
  const queryClient = useQueryClient();
  const [, navigate] = useRoute();
  const run = (work: Promise<unknown>) => {
    work.then(() => queryClient.invalidateQueries()).catch((error) => onMessage(describeError(error)));
  };
  const setRole = (library: LibraryDto, folder: string, role: FolderRoleDto | null) =>
    run(unwrap(commands.setFolderRole(library.id, folder, role)));
  /** `isCollection` is unknown where the folder's role is not loaded (breadcrumbs): then no check mark. */
  return (library: LibraryDto, folder: string, isCollection?: boolean): MenuItem[] =>
    folderMenuItems(isCollection, {
      open: () => navigate({ ...HOME, library: library.id, folder }),
      reveal: () => revealPath(library.root, folder, onMessage),
      copy: () => copyPath(library.root, folder, onMessage),
      setRole: (role) => setRole(library, folder, role),
      automaticCover: () => run(unwrap(commands.setFolderCover(library.id, folder, null))),
      scan: () => run(unwrap(commands.scanFolder(library.id, folder))),
    });
}

/** What a folder menu does. */
export type FolderMenuActions = {
  open: () => void;
  reveal: () => void;
  copy: () => void;
  setRole: (role: FolderRoleDto | null) => void;
  automaticCover: () => void;
  scan: () => void;
};

/** The items of a folder menu. */
export function folderMenuItems(isCollection: boolean | undefined, act: FolderMenuActions): MenuItem[] {
  return [
    { label: t("Open"), onSelect: act.open },
    { label: t("Show in folder"), onSelect: act.reveal },
    { label: t("Copy path"), onSelect: act.copy },
    "separator",
    { label: t("Treat as one model"), onSelect: () => act.setRole("model") },
    { label: t("Treat as collection"), checked: isCollection, onSelect: () => act.setRole("collection") },
    { label: t("Detect automatically"), onSelect: () => act.setRole(null) },
    "separator",
    { label: t("Automatic cover"), onSelect: act.automaticCover },
    { label: t("Scan this folder"), onSelect: act.scan },
    { hint: t("Rules for whole patterns (suggested collections) are listed in Settings › Libraries.") },
  ];
}

/** The menu of a library itself (its name in the sidebar, the first breadcrumb). */
export function useLibraryMenu(onMessage: (message: string) => void) {
  const [, navigate] = useRoute();
  return (library: LibraryDto): MenuItem[] => [
    { label: t("Show in folder"), onSelect: () => revealPath(library.root, "", onMessage) },
    { label: t("Scan for changes"), onSelect: () => void unwrap(commands.scanLibrary(library.id, false)).catch((error) => onMessage(describeError(error))) },
    { label: t("Library settings…"), onSelect: () => navigate(settingsRoute("library", library.id, "access")) },
  ];
}

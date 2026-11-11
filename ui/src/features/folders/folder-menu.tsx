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
  return (library: LibraryDto, folder: string, isCollection?: boolean): MenuItem[] => [
    { label: t("Open"), onSelect: () => navigate({ ...HOME, library: library.id, folder }) },
    { label: t("Show in folder"), onSelect: () => revealPath(library.root, folder, onMessage) },
    { label: t("Copy path"), onSelect: () => copyPath(library.root, folder, onMessage) },
    "separator",
    { label: t("Treat as one model"), onSelect: () => setRole(library, folder, "model") },
    { label: t("Treat as collection"), checked: isCollection, onSelect: () => setRole(library, folder, "collection") },
    { label: t("Detect automatically"), onSelect: () => setRole(library, folder, null) },
    "separator",
    { label: t("Automatic cover"), onSelect: () => run(unwrap(commands.setFolderCover(library.id, folder, null))) },
    { label: t("Scan this folder"), onSelect: () => run(unwrap(commands.scanFolder(library.id, folder))) },
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

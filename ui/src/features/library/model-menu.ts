import { useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap, type LibraryDto, type SlicerDto } from "../../ipc";
import { openAtLastPoint, type MenuItem } from "../../shared/context-menu";
import { copyPath, revealPath } from "../../shared/file-actions";
import { t } from "../../shared/i18n";
import { useRoute } from "../../shared/route";
import { useSlicers } from "../slicers/queries";
import { useLibraries } from "./queries";

/** What a model menu needs; marks and formats are left out where a view does not know them (then marks are not
 * offered, and "Open in" trusts the slicer). */
export type MenuModel = { id: number; library: number; path: string; favorite?: boolean; printed?: boolean; formats?: string[] };

/** What the menu acts with: the model's library, the default slicer, and the actions. */
export type ModelMenuEnv = {
  library: LibraryDto | undefined;
  slicer: SlicerDto | undefined;
  open: (id: number) => void;
  openInSlicer: (id: number) => void;
  reveal: (root: string, path: string) => void;
  copy: (root: string, path: string) => void;
  mark: (id: number, change: { favorite?: boolean; printed?: boolean }) => void;
  addToCollection: (id: number) => void;
  removeFromCollection: (collection: number, id: number) => void;
};

/** The items of a model menu. */
export function modelMenuItems(model: MenuModel, options: Options, env: ModelMenuEnv): MenuItem[] {
  const { library, slicer } = env;
  // A slicer that opens none of the model's formats (Lychee and a 3MF-only model, say) is not offered.
  const slicerFits = slicer !== undefined && (model.formats === undefined || model.formats.some((format) => slicer.extensions.includes(format.toLowerCase())));
  return [
    { label: t("Open"), onSelect: () => env.open(model.id) },
    { label: t("Open in {name}", { name: slicer?.name ?? "" }), hidden: !slicerFits, onSelect: () => env.openInSlicer(model.id) },
    { label: t("Show in folder"), hidden: !library, onSelect: () => library && env.reveal(library.root, model.path) },
    "separator",
    { label: t("Favorite"), checked: model.favorite, hidden: model.favorite === undefined, onSelect: () => env.mark(model.id, { favorite: !model.favorite }) },
    { label: t("Printed"), checked: model.printed, hidden: model.printed === undefined, onSelect: () => env.mark(model.id, { printed: !model.printed }) },
    { label: t("Add to collection…"), onSelect: () => env.addToCollection(model.id) },
    { label: t("Select"), hidden: !options.onSelect, onSelect: () => options.onSelect?.() },
    "separator",
    { label: t("Copy path"), hidden: !library, onSelect: () => library && env.copy(library.root, model.path) },
    "separator",
    {
      label: t("Remove from collection"),
      danger: true,
      hidden: options.collection === undefined,
      onSelect: () => options.collection !== undefined && env.removeFromCollection(options.collection, model.id),
    },
  ];
}

export type Options = {
  /** Selects this card (only where cards can be selected). */
  onSelect?: () => void;
  /** In a collection's view: the collection it can be removed from. */
  collection?: number;
};

/** The menu of a model card, the same in the grid, a collection and the tag check. */
export function useModelMenu(onMessage: (message: string) => void) {
  const queryClient = useQueryClient();
  const [, navigate] = useRoute();
  const libraries = useLibraries();
  const slicers = useSlicers();
  const fail = (error: unknown) => onMessage(describeError(error));
  const refresh = () => queryClient.invalidateQueries();
  const mark = async (id: number, change: { favorite?: boolean; printed?: boolean }) => {
    const detail = await unwrap(commands.getModel(id));
    if (detail) await unwrap(commands.updateModelMeta(id, { ...detail.meta, ...change }));
    await refresh();
  };
  const collections = async (id: number) => {
    const list = await queryClient.fetchQuery({ queryKey: ["collections"], queryFn: () => unwrap(commands.listCollections()) });
    const handMade = list.filter((collection) => collection.query === null);
    openAtLastPoint(handMade.length === 0
      ? [{ hint: t("Create a collection in the sidebar first.") }]
      : handMade.map((collection) => ({
          label: collection.name,
          onSelect: () => void unwrap(commands.addToCollection(collection.id, [id]))
            .then(() => { onMessage(t("Added to {name}", { name: collection.name })); return refresh(); })
            .catch(fail),
        })));
  };
  return (model: MenuModel, options: Options = {}): MenuItem[] =>
    modelMenuItems(model, options, {
      library: libraries.data?.find((candidate) => candidate.id === model.library),
      slicer: slicers.data?.[0],
      open: (id) => navigate({ view: "model", id }),
      openInSlicer: (id) => void unwrap(commands.openInSlicer(id, [], null)).then((name) => onMessage(t("Opened in {name}", { name }))).catch(fail),
      reveal: (root, path) => revealPath(root, path, onMessage),
      copy: (root, path) => copyPath(root, path, onMessage),
      mark: (id, change) => void mark(id, change).catch(fail),
      addToCollection: (id) => void collections(id).catch(fail),
      removeFromCollection: (collection, id) => void unwrap(commands.removeFromCollection(collection, [id])).then(refresh).catch(fail),
    });
}

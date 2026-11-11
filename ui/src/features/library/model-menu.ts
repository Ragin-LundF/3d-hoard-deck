import { useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap } from "../../ipc";
import { openAtLastPoint, type MenuItem } from "../../shared/context-menu";
import { copyPath, revealPath } from "../../shared/file-actions";
import { t } from "../../shared/i18n";
import { useRoute } from "../../shared/route";
import { useSlicers } from "../slicers/queries";
import { useLibraries } from "./queries";

/** What a model menu needs; marks are left out where a view does not know them (they are then not offered). */
export type MenuModel = { id: number; library: number; path: string; favorite?: boolean; printed?: boolean };

type Options = {
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
  return (model: MenuModel, options: Options = {}): MenuItem[] => {
    const library = libraries.data?.find((candidate) => candidate.id === model.library);
    const slicer = slicers.data?.[0];
    return [
      { label: t("Open"), onSelect: () => navigate({ view: "model", id: model.id }) },
      {
        label: t("Open in {name}", { name: slicer?.name ?? "" }),
        hidden: !slicer,
        onSelect: () => void unwrap(commands.openInSlicer(model.id, [], null)).then((name) => onMessage(t("Opened in {name}", { name }))).catch(fail),
      },
      { label: t("Show in folder"), hidden: !library, onSelect: () => library && revealPath(library.root, model.path, onMessage) },
      "separator",
      { label: t("Favorite"), checked: model.favorite, hidden: model.favorite === undefined, onSelect: () => void mark(model.id, { favorite: !model.favorite }).catch(fail) },
      { label: t("Printed"), checked: model.printed, hidden: model.printed === undefined, onSelect: () => void mark(model.id, { printed: !model.printed }).catch(fail) },
      { label: t("Add to collection…"), onSelect: () => void collections(model.id).catch(fail) },
      { label: t("Select"), hidden: !options.onSelect, onSelect: () => options.onSelect?.() },
      "separator",
      { label: t("Copy path"), hidden: !library, onSelect: () => library && copyPath(library.root, model.path, onMessage) },
      "separator",
      {
        label: t("Remove from collection"),
        danger: true,
        hidden: options.collection === undefined,
        onSelect: () => options.collection !== undefined && void unwrap(commands.removeFromCollection(options.collection, [model.id])).then(refresh).catch(fail),
      },
    ];
  };
}

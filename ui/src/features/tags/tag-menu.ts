import { useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap, type TagInfoDto_Serialize } from "../../ipc";
import type { MenuItem } from "../../shared/context-menu";
import { t } from "../../shared/i18n";
import { HOME, useRoute } from "../../shared/route";
import { toggleExclude, toggleInclude } from "./tag-query";

/** A tag rename asked for outside the tag browser; the browser opens with it ready (read once). */
let renameRequest: { library: number; tag: string } | null = null;

export function takeRenameRequest(library: number): string | null {
  const request = renameRequest?.library === library ? renameRequest.tag : null;
  renameRequest = null;
  return request;
}

type Options = {
  /** Where the tag bar filters the current view instead of opening a search. */
  onShow?: () => void;
  onExclude?: () => void;
  /** Renames inside the tag browser itself. */
  onRename?: () => void;
  onCheck?: () => void;
  /** Tag bar only: removes the tag from every model shown. */
  onRemoveAll?: () => void;
};

/** The menu of a tag, the same in the tag bar, the pinned tags and the tag browser. */
export function useTagMenu(onMessage: (message: string) => void) {
  const queryClient = useQueryClient();
  const [, navigate] = useRoute();
  return (library: number, tag: string, options: Options = {}): MenuItem[] => {
    const known = queryClient.getQueryData<TagInfoDto_Serialize[]>(["tags", library])?.find((info) => info.name.toLowerCase() === tag.toLowerCase());
    const pinned = known?.pinned ?? false;
    return [
      { label: t("Show models with this tag"), onSelect: options.onShow ?? (() => navigate({ ...HOME, library, q: toggleInclude("", tag) })) },
      { label: t("Exclude this tag"), onSelect: options.onExclude ?? (() => navigate({ ...HOME, library, q: toggleExclude("", tag) })) },
      {
        label: pinned ? t("Unpin from the sidebar") : t("Pin to the sidebar"),
        onSelect: () => void unwrap(commands.setTagsPinned(library, [tag], !pinned))
          .then(() => queryClient.invalidateQueries({ queryKey: ["tags", library] }))
          .catch((error) => onMessage(describeError(error))),
      },
      {
        label: t("Rename…"),
        onSelect: options.onRename ?? (() => {
          renameRequest = { library, tag };
          navigate({ view: "tags", library });
        }),
      },
      { label: t("Check this tag…"), hidden: !options.onCheck, onSelect: () => options.onCheck?.() },
      "separator",
      { label: t("Remove from all here…"), danger: true, hidden: !options.onRemoveAll, onSelect: () => options.onRemoveAll?.() },
    ];
  };
}

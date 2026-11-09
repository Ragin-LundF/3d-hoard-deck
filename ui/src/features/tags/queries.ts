import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { commands, unwrap } from "../../ipc";

/** Every tag of a library with count, group and pin. */
export function useTags(library: number | null) {
  return useQuery({
    queryKey: ["tags", library],
    enabled: library !== null,
    queryFn: () => unwrap(commands.listTags(library ?? 0)),
  });
}

export function useTagGroups(library: number | null) {
  return useQuery({
    queryKey: ["tag-groups", library],
    enabled: library !== null,
    queryFn: () => unwrap(commands.listTagGroups(library ?? 0)),
  });
}

/** The most used tags of what is shown: the folder and below, narrowed by the search. */
export function useTagFacets(library: number, folder: string, query: string, limit: number) {
  return useQuery({
    queryKey: ["tag-facets", library, folder, query, limit],
    queryFn: () => unwrap(commands.tagFacets(library, folder, query, limit)),
    // Keeps the chips in place while the next counts load, so the bar does not flicker.
    placeholderData: keepPreviousData,
  });
}

/** Group colours offered in the tag browser. */
export const GROUP_COLORS = ["#4f8cff", "#22b8a7", "#4cc38a", "#e8b04b", "#ff7a45", "#e5484d", "#c063e6", "#8e96a6"];

import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { commands, unwrap, type CursorDto, type SortDto } from "../../ipc";

const PAGE = 120;

export function useLibraries() {
  return useQuery({ queryKey: ["libraries"], queryFn: () => unwrap(commands.listLibraries()) });
}

export function useFolderPage(library: number | null, folder: string, recursive: boolean, sort: SortDto) {
  return useInfiniteQuery({
    queryKey: ["browse", library, folder, recursive, sort],
    enabled: library !== null,
    initialPageParam: null as CursorDto | null,
    queryFn: ({ pageParam }) =>
      unwrap(commands.browseFolder({ library: library ?? 0, folder, recursive, sort, cursor: pageParam, limit: PAGE })),
    getNextPageParam: (page) => page.next ?? undefined,
  });
}

export function useSearch(library: number | null, text: string, scope: string) {
  return useInfiniteQuery({
    queryKey: ["search", library, text, scope],
    enabled: text.trim().length > 0,
    initialPageParam: 0,
    queryFn: ({ pageParam }) =>
      unwrap(commands.searchModels({ text, library, scope: scope || null, offset: pageParam, limit: PAGE })),
    getNextPageParam: (page, pages) => {
      const loaded = pages.reduce((sum, p) => sum + p.models.length, 0);
      return loaded < page.total ? loaded : undefined;
    },
  });
}

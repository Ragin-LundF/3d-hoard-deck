import { useQuery } from "@tanstack/react-query";
import { commands, unwrap } from "../../ipc";
import { Icon } from "../../shared/icons";
import type { LibraryRoute } from "../../shared/route";
import { t, formatNumber } from "../../shared/i18n";

/** Shown in the sidebar: up to this many tags, most used first. */
const SIDEBAR_TAGS = 12;

type Props = {
  library: number;
  /** The current route when this library is shown, else `null`. */
  route: LibraryRoute | null;
  onNavigate: (patch: Partial<LibraryRoute>) => void;
};

/** Library entry points in the sidebar: folders, all models, favorites, printed and the most used tags. */
export function LibraryNav({ library, route, onNavigate }: Props) {
  const tags = useQuery({ queryKey: ["tags", library], queryFn: () => unwrap(commands.listTags(library)) });
  const on = (check: (route: LibraryRoute) => boolean) => (route && check(route) ? "nav on" : "nav");
  const query = (q: string) => `nav${route?.q === q ? " on" : ""}`;
  const tagQuery = (name: string) => (name.includes(" ") ? `tag:"${name}"` : `tag:${name}`);
  return (
    <>
      <button className={on((r) => !r.q && !r.folder && !r.all)} onClick={() => onNavigate({})}>
        <Icon name="folder" />{t("Browse folders")}
      </button>
      <button className={on((r) => !r.q && !r.folder && r.all)} onClick={() => onNavigate({ all: true })}>
        <Icon name="grid" />{t("All models")}
      </button>
      <button className={query("is:favorite")} onClick={() => onNavigate({ q: "is:favorite" })}>
        <Icon name="star" />{t("Favorites")}
      </button>
      <button className={query("is:printed")} onClick={() => onNavigate({ q: "is:printed" })}>
        <Icon name="check" />{t("Printed")}
      </button>
      {(tags.data?.length ?? 0) > 0 && <h4 className="subhead">{t("Tags")}</h4>}
      {tags.data?.slice(0, SIDEBAR_TAGS).map((tag) => (
        <button key={tag.name} className={query(tagQuery(tag.name))} onClick={() => onNavigate({ q: tagQuery(tag.name) })}>
          <Icon name="tag" /><span className="lbl">{tag.name}</span><span className="n">{formatNumber(tag.count)}</span>
        </button>
      ))}
    </>
  );
}

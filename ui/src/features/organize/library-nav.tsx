import { useTags } from "../tags/queries";
import { Icon } from "../../shared/icons";
import type { LibraryRoute } from "../../shared/route";
import { t, formatNumber } from "../../shared/i18n";

type Props = {
  library: number;
  /** The current route when this library is shown, else `null`. */
  route: LibraryRoute | null;
  onNavigate: (patch: Partial<LibraryRoute>) => void;
  /** Opens the tag browser of this library. */
  onTags: () => void;
  tagsOpen: boolean;
};

/** Library entry points in the sidebar: folders, all models, favorites, printed and the tag browser. */
export function LibraryNav({ library, route, onNavigate, onTags, tagsOpen }: Props) {
  const tags = useTags(library);
  const on = (check: (route: LibraryRoute) => boolean) => (route && check(route) ? "nav on" : "nav");
  const query = (q: string) => `nav${route?.q === q ? " on" : ""}`;
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
      <button className={tagsOpen ? "nav on" : "nav"} onClick={onTags}>
        <Icon name="tag" /><span className="lbl">{t("Tags")}</span><span className="n">{formatNumber(tags.data?.length ?? 0)}</span>
      </button>
    </>
  );
}

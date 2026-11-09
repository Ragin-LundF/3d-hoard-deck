import { useQuery } from "@tanstack/react-query";
import { commands, unwrap } from "../../ipc";
import { useAiEnabled } from "../ai/queries";
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
  /** Opens the AI tag review; the entry is there only while AI is set up. */
  onReview: () => void;
  reviewOpen: boolean;
};

/** Library entry points in the sidebar: folders, all models, favorites, printed and the tag browser. */
export function LibraryNav({ library, route, onNavigate, onTags, tagsOpen, onReview, reviewOpen }: Props) {
  const tags = useTags(library);
  const ai = useAiEnabled();
  const pending = useQuery({ queryKey: ["pending", library, ""], queryFn: () => unwrap(commands.pendingSuggestions(library, "")), enabled: ai });
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
      {ai && (
        <button className={reviewOpen ? "nav on" : "nav"} onClick={onReview}>
          <Icon name="layers" /><span className="lbl">{t("Review AI tags")}</span><span className="n">{formatNumber(pending.data ?? 0)}</span>
        </button>
      )}
    </>
  );
}

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { commands, unwrap, type LibraryDto } from "../../ipc";
import { Icon, type IconName } from "../../shared/icons";
import { t } from "../../shared/i18n";
import type { SettingsRoute, SettingsTab } from "../../shared/route";
import { settingsRoute } from "../../shared/route";
import { useAiSettings } from "../ai/queries";
import type { ScanStatus } from "../library/use-scan-status";
import { useSlicers } from "../slicers/queries";
import { useUpdate } from "../update/use-update";
import { aiDraft, tagRuleDraft } from "./drafts";

type Props = {
  route: SettingsRoute;
  libraries: LibraryDto[];
  scans: Record<number, ScanStatus>;
  onNavigate: (route: SettingsRoute) => void;
  onBack: () => void;
};

type Entry = { label: string; tab: SettingsTab; sub?: string; perLibrary?: boolean };

/** What "Find a setting" knows: one entry per setting, pointing to its page (and tab). */
const ENTRIES: Entry[] = [
  { label: t("Add library folder"), tab: "libraries" },
  { label: t("What 3D Hoard Deck may change"), tab: "library", sub: "access", perLibrary: true },
  { label: t("Where tags, notes and names are saved"), tab: "library", sub: "access", perLibrary: true },
  { label: t("Suggested collections"), tab: "library", sub: "rules", perLibrary: true },
  { label: t("Folder rules"), tab: "library", sub: "rules", perLibrary: true },
  { label: t("Rebuild from metadata files"), tab: "library", sub: "import", perLibrary: true },
  { label: t("Import from Orynt3D"), tab: "library", sub: "import", perLibrary: true },
  { label: t("Recent file changes"), tab: "files" },
  { label: t("Tag rules"), tab: "tags", sub: "rules" },
  { label: t("Show pinned tags in the sidebar"), tab: "tags", sub: "display" },
  { label: t("AI service"), tab: "ai" },
  { label: t("API key"), tab: "ai" },
  { label: t("Find installed slicers"), tab: "slicers" },
  { label: t("Language of the app"), tab: "general" },
  { label: t("Theme"), tab: "general" },
  { label: t("Index folder"), tab: "index" },
  { label: t("Check for updates on start"), tab: "updates" },
];

/** While in Settings, this replaces the folder tree: back, search, and the pages with their state at a glance. */
export function SettingsNav({ route, libraries, scans, onNavigate, onBack }: Props) {
  const [find, setFind] = useState("");
  const rules = useQuery({ queryKey: ["tagRules"], queryFn: () => unwrap(commands.tagRules()) });
  const ai = useAiSettings();
  const slicers = useSlicers();
  const update = useUpdate();
  const ruleDraft = tagRuleDraft.use();
  const aiChanged = aiDraft.use();
  const problems = libraries.some((library) => (scans[library.id]?.unreadable ?? 0) > 0);
  const managed = libraries.some((library) => library.access === "managed");
  const aiOn = (ai.data?.settings.provider ?? "off") !== "off";
  const pages: { tab: SettingsTab; icon: IconName; label: string; extra?: React.ReactNode; section?: string; hidden?: boolean }[] = [
    { tab: "overview", icon: "grid", label: t("Overview") },
    { tab: "libraries", icon: "drive", label: t("Libraries"), section: t("Libraries"), extra: problems ? <span className="dot warn" /> : <span className="n">{libraries.length}</span> },
    { tab: "files", icon: "refresh", label: t("File changes"), hidden: !managed },
    { tab: "tags", icon: "tag", label: t("Tags & rules"), section: t("Organize"), extra: ruleDraft ? <span className="dot draft" /> : <span className="n">{rules.data?.filter((rule) => rule.enabled).length ?? 0}</span> },
    { tab: "ai", icon: "layers", label: t("AI suggestions"), extra: aiChanged ? <span className="dot draft" /> : <span className={`pill${aiOn ? " ok-pill" : ""}`}>{aiOn ? t("On") : t("Off")}</span> },
    { tab: "slicers", icon: "box", label: t("Slicers"), section: t("Open"), extra: <span className="n">{slicers.data?.length ?? 0}</span> },
    { tab: "general", icon: "settings", label: t("General"), section: t("App") },
    { tab: "index", icon: "database", label: t("Index & storage") },
    { tab: "updates", icon: "refresh", label: t("Updates"), extra: update.info && !update.hidden ? <span className="dot" /> : null },
  ];
  const needle = find.trim().toLowerCase();
  const go = (entry: Entry) => {
    const only = libraries.length === 1 ? libraries[0]!.id : null;
    onNavigate(entry.perLibrary && only === null ? settingsRoute("libraries") : settingsRoute(entry.tab, entry.perLibrary ? only : null, entry.sub ?? ""));
    setFind("");
  };
  const active = route.tab === "library" ? "libraries" : route.tab;

  return (
    <div className="side-scroll setnav">
      <div className="sec">
        <button className="nav" onClick={onBack}><Icon name="back" />{t("Back to library")}</button>
        <input className="set-find" value={find} placeholder={t("Find a setting")} aria-label={t("Find a setting")} onChange={(event) => setFind(event.target.value)} />
      </div>
      {needle ? (
        <div className="sec">
          {ENTRIES.filter((entry) => entry.label.toLowerCase().includes(needle)).map((entry) => (
            <button key={entry.label} className="nav" onClick={() => go(entry)}><Icon name="search" /><span className="lbl">{entry.label}</span></button>
          ))}
        </div>
      ) : (
        <div className="sec">
          {pages.filter((page) => !page.hidden).map((page) => (
            <div key={page.tab}>
              {page.section && <h3>{page.section}</h3>}
              <button className={`nav${active === page.tab ? " on" : ""}`} aria-current={active === page.tab ? "page" : undefined} onClick={() => onNavigate(settingsRoute(page.tab))}>
                <Icon name={page.icon} /><span className="lbl">{page.label}</span>{page.extra}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

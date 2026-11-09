import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { getVersion } from "@tauri-apps/api/app";
import { commands, describeError, unwrap, type LibraryDto } from "../../ipc";
import { Icon, type IconName } from "../../shared/icons";
import { languageChoice, plural, setLanguage, t, type LanguageChoice } from "../../shared/i18n";
import { settingsRoute, type SettingsRoute, type SettingsTab } from "../../shared/route";
import type { Theme } from "../../shared/use-theme";
import { AiSettings } from "../ai/ai-settings";
import { useAiSettings } from "../ai/queries";
import { FileJournal } from "../files/file-journal";
import { isScanning, type ScanStatus } from "../library/use-scan-status";
import { useSlicers } from "../slicers/queries";
import { TagRuleSettings } from "../tagging/tag-rule-settings";
import { setShowPinnedTags, useShowPinnedTags } from "../tags/pinned-tags";
import { UpdateCard } from "../update/update-pill";
import { UpdateSettings } from "../update/update-settings";
import { useUpdate } from "../update/use-update";
import { IndexSettings } from "./index-settings";
import { ACCESS_LABELS, LibrarySettings, type LibrarySub } from "./library-settings";
import { SlicerSettings } from "./slicer-settings";
import { confirmDiscard } from "./drafts";

type Props = {
  route: SettingsRoute;
  navigate: (route: SettingsRoute) => void;
  libraries: LibraryDto[];
  scans: Record<number, ScanStatus>;
  theme: Theme;
  onTheme: (theme: Theme) => void;
  onTags: (library: number) => void;
  onError: (message: string) => void;
};

/** Asks for a folder, registers it (nothing is written into it until the user allows it) and starts the first scan. */
export function useAddLibrary(onError: (message: string) => void) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      const root = await open({ directory: true, title: t("Choose a library folder") });
      if (typeof root !== "string") return null;
      const name = root.split(/[\\/]/).filter(Boolean).pop() ?? root;
      const library = await unwrap(commands.addLibrary(name, root));
      await unwrap(commands.scanLibrary(library.id, false));
      return library;
    },
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["libraries"] }),
    onError: (error) => onError(describeError(error)),
  });
}

function PageHead({ title, lead, children }: { title: string; lead?: string; children?: React.ReactNode }) {
  return (
    <div className="phead">
      <div className="t"><h1>{title}</h1>{lead && <p>{lead}</p>}</div>
      {children}
    </div>
  );
}

/** Settings as pages (the sidebar lists them); each page holds the existing setting components. */
export function SettingsView({ route, navigate, libraries, scans, theme, onTheme, onTags, onError }: Props) {
  const go = (tab: SettingsTab, lib: number | null = null, sub = "") => navigate(settingsRoute(tab, lib, sub));
  const library = libraries.find((item) => item.id === route.lib);
  let page: React.ReactNode;
  switch (route.tab) {
    case "libraries":
      page = <LibrariesPage libraries={libraries} scans={scans} onOpen={(id) => go("library", id, "access")} onError={onError} />;
      break;
    case "library":
      page = library
        ? <LibrarySettings library={library} scan={scans[library.id]} sub={(["access", "rules", "import"].includes(route.sub) ? route.sub : "access") as LibrarySub}
            onSub={(sub) => navigate(settingsRoute("library", library.id, sub))} onError={onError} />
        : <LibrariesPage libraries={libraries} scans={scans} onOpen={(id) => go("library", id, "access")} onError={onError} />;
      break;
    case "files":
      page = <><PageHead title={t("File changes")} lead={t("Renames and moves on disk. Undo puts the files and their names, tags and notes back.")} /><FileJournal libraries={libraries} onError={onError} /></>;
      break;
    case "tags":
      page = <TagsPage sub={route.sub === "display" ? "display" : "rules"} onSub={(sub) => go("tags", null, sub)} libraries={libraries} onTags={onTags} onError={onError} />;
      break;
    case "ai":
      page = <><PageHead title={t("AI suggestions")} lead={t("Tags suggested from the preview picture and the name. Nothing changes until you keep a tag.")} /><AiSettings libraries={libraries} onError={onError} /></>;
      break;
    case "slicers":
      page = <><PageHead title={t("Slicers")} lead={t("“Open in” uses the first slicer that opens the files; the others are in its menu. Drag to change the order.")} /><SlicerSettings onError={onError} /></>;
      break;
    case "general":
      page = <GeneralPage theme={theme} onTheme={onTheme} />;
      break;
    case "index":
      page = (
        <>
          <PageHead title={t("Index & storage")} />
          <IndexSettings onError={onError} />
          <div className="callout"><Icon name="database" /><span>{t("Libraries that keep tags only in the index have them only here. Back up the index folder to keep your work.")}</span></div>
        </>
      );
      break;
    case "updates":
      page = <UpdatesPage scanning={libraries.some((item) => isScanning(scans[item.id]))} />;
      break;
    default:
      page = <OverviewPage libraries={libraries} scans={scans} theme={theme} go={go} />;
  }
  // One level up within Settings: a library's page goes to the list, every other page to the overview.
  const up = route.tab === "overview" ? null : route.tab === "library" ? settingsRoute("libraries") : settingsRoute();
  return (
    <div className="setpage">
      {up && <button className="set-back" onClick={() => navigate(up)}><Icon name="back" />{route.tab === "library" ? t("Libraries") : t("Overview")}</button>}
      {page}
    </div>
  );
}

function LibrariesPage({ libraries, scans, onOpen, onError }: { libraries: LibraryDto[]; scans: Record<number, ScanStatus>; onOpen: (id: number) => void; onError: (message: string) => void }) {
  const addLibrary = useAddLibrary(onError);
  return (
    <>
      <PageHead title={t("Libraries")} lead={t("Folders 3D Hoard Deck indexes. Files stay where they are; each library decides what the app may change in it.")}>
        <button className="btn primary" onClick={() => addLibrary.mutate()} disabled={addLibrary.isPending}><Icon name="plus" />{t("Add library folder")}</button>
      </PageHead>
      <div className="panel">
        {libraries.map((library) => {
          const scan = scans[library.id];
          return (
            <button key={library.id} className="set-lib" onClick={() => onOpen(library.id)}>
              <span className="ic"><Icon name="drive" /></span>
              <span className="t">
                <b>{library.name}</b>
                <span className="mono">{library.root}</span>
                <span className="meta">
                  <span className={`pill${library.access === "readOnly" ? " lock" : ""}`}>{ACCESS_LABELS[library.access]}</span>
                  <span className="pill">{library.metadata === "indexOnly" ? t("Tags in the index") : t("Tags in metadata files")}</span>
                  {isScanning(scan) && <span className="pill">{t("Scanning…")}</span>}
                  {!isScanning(scan) && (scan?.unreadable ?? 0) > 0 && <span className="pill err">{plural(scan!.unreadable, "{count} unreadable", "{count} unreadable")}</span>}
                </span>
              </span>
              <Icon name="chevron" />
            </button>
          );
        })}
      </div>
    </>
  );
}

function TagsPage({ sub, onSub, libraries, onTags, onError }: { sub: "rules" | "display"; onSub: (sub: string) => void; libraries: LibraryDto[]; onTags: (library: number) => void; onError: (message: string) => void }) {
  const pinned = useShowPinnedTags();
  return (
    <>
      <PageHead title={t("Tags & rules")} lead={t("Tags from folder and file names, such as 32mm, presupported or the creator. Only whole words count, and nothing is written into your libraries.")} />
      <div className="subtabs" role="tablist">
        <button role="tab" aria-selected={sub === "rules"} className={sub === "rules" ? "on" : ""} onClick={() => onSub("rules")}>{t("Tag rules")}</button>
        <button role="tab" aria-selected={sub === "display"} className={sub === "display" ? "on" : ""} onClick={() => onSub("display")}>{t("Display")}</button>
      </div>
      {sub === "rules" ? <TagRuleSettings onError={onError} /> : (
        <div className="panel">
          <label className="prow2">
            <div className="t"><b>{t("Show pinned tags in the sidebar")}</b><span>{t("Pin tags in the tag browser; they then appear below the folders.")}</span></div>
            <input type="checkbox" checked={pinned} onChange={(event) => setShowPinnedTags(event.target.checked)} />
          </label>
          {libraries.map((library) => (
            <div className="prow2" key={library.id}>
              <div className="t"><b>{t("Tag browser of {name}", { name: library.name })}</b><span>{t("Merge, rename, group and pin tags.")}</span></div>
              <button className="btn" onClick={() => onTags(library.id)}><Icon name="tag" />{t("Open")}</button>
            </div>
          ))}
        </div>
      )}
    </>
  );
}

function GeneralPage({ theme, onTheme }: { theme: Theme; onTheme: (theme: Theme) => void }) {
  return (
    <>
      <PageHead title={t("General")} />
      <div className="panel">
        <div className="prow2">
          <div className="t"><b>{t("Language of the app")}</b><span>{t("The app restarts its window to switch.")}</span></div>
          <select
            className="sel"
            aria-label={t("Language")}
            value={languageChoice()}
            onChange={(event) => {
              const select = event.target;
              const choice = select.value as LanguageChoice;
              // Switching reloads the window, which would lose unsaved tag rules or AI settings.
              void confirmDiscard().then((yes) => (yes ? setLanguage(choice) : (select.value = languageChoice())));
            }}
          >
            <option value="system">{t("Like the system")}</option>
            <option value="en">English</option>
            <option value="de">Deutsch</option>
          </select>
        </div>
        <div className="prow2">
          <div className="t"><b>{t("Theme")}</b></div>
          <div className="seg" role="group" aria-label={t("Theme")}>
            <button className={theme === "light" ? "on" : ""} aria-pressed={theme === "light"} onClick={() => onTheme("light")}><Icon name="sun" />{t("Light")}</button>
            <button className={theme === "dark" ? "on" : ""} aria-pressed={theme === "dark"} onClick={() => onTheme("dark")}><Icon name="moon" />{t("Dark")}</button>
          </div>
        </div>
      </div>
    </>
  );
}

function UpdatesPage({ scanning }: { scanning: boolean }) {
  const update = useUpdate();
  return (
    <>
      <PageHead title={t("Updates")} />
      {update.info && <UpdateCard scanning={scanning} inline onClose={() => {}} />}
      <UpdateSettings />
    </>
  );
}

type OverviewProps = { libraries: LibraryDto[]; scans: Record<number, ScanStatus>; theme: Theme; go: (tab: SettingsTab, lib?: number | null, sub?: string) => void };

function OverviewPage({ libraries, scans, theme, go }: OverviewProps) {
  const rules = useQuery({ queryKey: ["tagRules"], queryFn: () => unwrap(commands.tagRules()) });
  const ai = useAiSettings();
  const slicers = useSlicers();
  const location = useQuery({ queryKey: ["index-location"], queryFn: () => unwrap(commands.indexLocation()) });
  const update = useUpdate();
  const pinned = useShowPinnedTags();
  const [version, setVersion] = useState("");
  useEffect(() => void getVersion().then(setVersion, () => setVersion("")), []);
  const troubled = libraries.filter((library) => !isScanning(scans[library.id]) && (scans[library.id]?.unreadable ?? 0) > 0);
  const provider = ai.data?.settings.provider ?? "off";
  const active = rules.data?.filter((rule) => rule.enabled).length ?? 0;
  const cards: { tab: SettingsTab; icon: IconName; title: string; lines: React.ReactNode[] }[] = [
    { tab: "libraries", icon: "drive", title: t("Libraries"), lines: libraries.map((library) => <><b>{library.name}</b> · {ACCESS_LABELS[library.access]}</>) },
    { tab: "tags", icon: "tag", title: t("Tags & rules"), lines: [plural(active, "{count} tag rule on", "{count} tag rules on"), pinned ? t("Pinned tags in the sidebar") : t("Pinned tags hidden")] },
    { tab: "ai", icon: "layers", title: t("AI suggestions"), lines: [provider === "off" ? <span className="off">{t("Off: nothing is sent anywhere")}</span> : t("On: {service}", { service: provider })] },
    { tab: "slicers", icon: "box", title: t("Slicers"), lines: [slicers.data?.[0] ? t("Default: {name}", { name: slicers.data[0].name }) : t("No slicers yet"), plural(slicers.data?.length ?? 0, "{count} slicer", "{count} slicers")] },
    { tab: "general", icon: "settings", title: t("General"), lines: [languageChoice() === "system" ? t("Language like the system") : languageChoice() === "de" ? "Deutsch" : "English", theme === "dark" ? t("Dark theme") : t("Light theme")] },
    { tab: "index", icon: "database", title: t("Index & storage"), lines: [<span className="mono">{location.data ?? ""}</span>] },
    { tab: "updates", icon: "refresh", title: t("Updates"), lines: [update.info ? t("Version {version} is available", { version: update.info.version }) : t("Installed: {version}", { version })] },
  ];
  return (
    <>
      <PageHead title={t("Settings")} />
      {(troubled.length > 0 || update.info) && (
        <div className="attn">
          {troubled.map((library) => (
            <div className="callout warn" key={library.id}>
              <Icon name="drive" />
              <span className="grow">{plural(scans[library.id]!.unreadable, "{count} folder or file in {name} could not be read.", "{count} folders or files in {name} could not be read.").replace("{name}", library.name)}</span>
              <button className="btn ghost" onClick={() => go("library", library.id, "access")}>{t("Show")}</button>
            </div>
          ))}
          {update.info && (
            <div className="callout">
              <Icon name="refresh" />
              <span className="grow">{t("Version {version} is available", { version: update.info.version })}</span>
              <button className="btn ghost" onClick={() => go("updates")}>{t("Show")}</button>
            </div>
          )}
        </div>
      )}
      <div className="set-cards">
        {cards.map((card) => (
          <button key={card.tab} className="set-card" onClick={() => go(card.tab)}>
            <span className="h"><span className="ic"><Icon name={card.icon} /></span>{card.title}<span className="go"><Icon name="chevron" /></span></span>
            <ul>{card.lines.map((line, index) => <li key={index}>{line}</li>)}</ul>
          </button>
        ))}
      </div>
    </>
  );
}

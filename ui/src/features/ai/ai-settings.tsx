import { useEffect, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap, type AiProviderDto, type AiSettingsDto, type AiTestDto, type LibraryDto } from "../../ipc";
import { Icon } from "../../shared/icons";
import { formatNumber, plural, t } from "../../shared/i18n";
import { useAiSettings, useSuggestProgress } from "./queries";
import { aiDraft } from "../settings/drafts";
import { SaveBar } from "../settings/save-bar";

const PROVIDERS: { value: AiProviderDto; label: string }[] = [
  { value: "off", label: t("Off") },
  { value: "lmStudio", label: "LM Studio" },
  { value: "ollama", label: "Ollama" },
  { value: "openAi", label: "OpenAI" },
  { value: "anthropic", label: "Anthropic (Claude)" },
  { value: "custom", label: t("Other OpenAI-compatible server") },
];

const DEFAULT_URLS: Record<AiProviderDto, string> = {
  off: "",
  lmStudio: "http://localhost:1234/v1",
  ollama: "http://localhost:11434/v1",
  openAi: "https://api.openai.com/v1",
  anthropic: "https://api.anthropic.com/v1",
  custom: "",
};

const REMOTE: AiProviderDto[] = ["openAi", "anthropic"];
const NEEDS_KEY: AiProviderDto[] = ["openAi", "anthropic"];

type Props = { libraries: LibraryDto[]; onError: (message: string) => void };

/**
 * AI tag suggestions, modelled on the DMS: off by default, a local OpenAI-compatible server or a cloud service, the
 * model from the server's list, and a test before saving. The key goes to the system keychain and never comes back.
 * Edits are a draft (kept when switching settings pages) until saved in the save bar.
 */
export function AiSettings({ libraries, onError }: Props) {
  const view = useAiSettings();
  const queryClient = useQueryClient();
  const draft = aiDraft.use();
  const [test, setTest] = useState<AiTestDto | null>(null);
  const progress = useSuggestProgress();
  useEffect(() => setTest(null), [draft?.settings.provider, draft?.settings.baseUrl]);
  const save = useMutation({
    mutationFn: (next: { settings: AiSettingsDto; key: string | null }) => unwrap(commands.saveAiSettings(next.settings, next.key)),
    onSuccess: () => {
      aiDraft.set(null);
      void queryClient.invalidateQueries({ queryKey: ["ai"] });
    },
    onError: (error) => onError(describeError(error)),
  });
  if (!view.data) return null;
  const stored = view.data.settings;
  const settings = draft?.settings ?? stored;
  const key = draft?.key ?? null;
  const update = (next: AiSettingsDto, nextKey: string | null) =>
    aiDraft.set(JSON.stringify(next) === JSON.stringify(stored) && nextKey === null ? null : { settings: next, key: nextKey });
  const change = (patch: Partial<AiSettingsDto>) => update({ ...settings, ...patch }, key);
  const setKey = (text: string | null) => update(settings, text);
  const on = settings.provider !== "off";
  const url = settings.baseUrl.trim() || DEFAULT_URLS[settings.provider];
  const remote = REMOTE.includes(settings.provider) || (settings.provider === "custom" && url !== "" && !/localhost|127\.0\.0\.1|\[::1\]/.test(url));
  const sameProvider = stored.provider === settings.provider;
  const keyNote = !sameProvider ? null
    : view.data.hasKey ? t("A key is stored in the system keychain. Type a new one to replace it, or remove it.")
    : view.data.keyFromEnvironment ? t("The key is read from the environment variable.")
    : null;
  const saveBar = draft && <SaveBar what={t("AI suggestions")} saving={save.isPending} onSave={() => save.mutate(draft)} onDiscard={() => aiDraft.set(null)} />;

  if (!on) {
    return (
      <>
        <div className="panel">
          <div className="set-empty">
            <span className="big"><Icon name="tag" /></span>
            <div className="t">
              <b>{t("AI suggestions are off")}</b>
              <p>{t("Looks at the preview picture and the name and suggests tags. It only suggests; you keep or dismiss. When off, the app shows no AI feature and sends nothing anywhere.")}</p>
              <div className="actions">
                {PROVIDERS.filter((provider) => provider.value !== "off").map((provider) => (
                  <button key={provider.value} className="btn" onClick={() => change({ provider: provider.value, baseUrl: "", model: "" })}>{provider.label}</button>
                ))}
              </div>
            </div>
          </div>
        </div>
        {saveBar}
      </>
    );
  }

  return (
    <>
      <div className="phead-actions">
        <button className="btn ghost" onClick={() => change({ provider: "off" })}>{t("Turn off")}</button>
      </div>
      <h2>{t("Connection")}</h2>
      <div className="panel">
        <div className="prow2">
          <div className="t"><b>{t("AI service")}</b></div>
          <select className="sel" aria-label={t("AI service")} value={settings.provider} onChange={(event) => change({ provider: event.target.value as AiProviderDto, baseUrl: "", model: "" })}>
            {PROVIDERS.map((provider) => <option key={provider.value} value={provider.value}>{provider.label}</option>)}
          </select>
        </div>
        <div className="prow2 stack">
          <label className="field">
            <span>{t("Server address")}</span>
            <input value={settings.baseUrl} placeholder={DEFAULT_URLS[settings.provider] || "http://server:port/v1"} onChange={(event) => change({ baseUrl: event.target.value })} />
          </label>
          <label className="field">
            <span>{t("Model")}</span>
            <input value={settings.model} placeholder={settings.provider === "anthropic" ? "claude-haiku-5-5" : t("First model the server lists")} onChange={(event) => change({ model: event.target.value })} />
          </label>
          {test && test.models.length > 0 && (
            <div className="tr-result">
              {test.models.slice(0, 30).map((model) => (
                <button key={model} className={`chip${settings.model === model ? " inc" : ""}`} onClick={() => change({ model })}>{model}</button>
              ))}
            </div>
          )}
          <label className="field">
            <span>{t("API key")}{NEEDS_KEY.includes(settings.provider) ? "" : ` ${t("(only if the server needs one)")}`}</span>
            <input type="password" autoComplete="off" value={key ?? ""} placeholder={view.data.hasKey && sameProvider ? "••••••••" : ""} onChange={(event) => setKey(event.target.value)} />
          </label>
          {keyNote && (
            <span className="hint">
              {keyNote}{" "}
              {view.data.hasKey && <button className="linkish" onClick={() => setKey("")}>{t("Remove the key")}</button>}
            </span>
          )}
          <TestButton settings={settings} keyText={key} onTested={setTest} onError={onError} />
          {test && (
            <span className="okline" aria-live="polite">
              <Icon name="check" /> {t("{model} answered in {seconds} s", { model: test.model, seconds: formatNumber(test.millis / 1000, 1) })} · {t("Tags for “Red dragon bust”: {tags}", { tags: test.tags.join(", ") || "–" })}
            </span>
          )}
        </div>
      </div>
      <h2>{t("What is sent")}</h2>
      <div className="panel">
        <div className="prow2 stack">
          {remote && <span className="warn-text">{t("Preview pictures and model names are sent to this service. Model files never are.")}</span>}
          <label className="tr-check"><input type="checkbox" checked={settings.sendImage} onChange={(event) => change({ sendImage: event.target.checked })} />{t("Send the preview picture (a small JPEG)")}</label>
          <label className="tr-check"><input type="checkbox" checked={settings.sendPath} onChange={(event) => change({ sendPath: event.target.checked })} />{t("Send the folder path")}</label>
          <div className="tr-line">
            <label className="field small"><span>{t("Tags per model")}</span>
              <input type="number" min={1} max={10} value={settings.maxTags} onChange={(event) => change({ maxTags: Number(event.target.value) })} /></label>
            <label className="field small"><span>{t("Timeout (seconds)")}</span>
              <input type="number" min={5} max={600} value={settings.timeoutSeconds} onChange={(event) => change({ timeoutSeconds: Number(event.target.value) })} /></label>
          </div>
          <details>
            <summary>{t("Own instructions (advanced)")}</summary>
            <textarea className="notes" value={settings.prompt} placeholder={t("Empty: the built-in instructions. {max}, {tags}, {name} and {path} are filled in.")} onChange={(event) => change({ prompt: event.target.value })} />
          </details>
        </div>
      </div>
      {stored.provider !== "off" && !draft && (
        <>
          <h2>{t("Suggest tags now")}</h2>
          <div className="panel">
            {libraries.map((library) => (
              <RunRow key={library.id} library={library} running={progress?.running ?? false} onError={onError} />
            ))}
          </div>
        </>
      )}
      {saveBar}
    </>
  );
}

type TestProps = { settings: AiSettingsDto; keyText: string | null; onTested: (test: AiTestDto) => void; onError: (message: string) => void };

function TestButton({ settings, keyText, onTested, onError }: TestProps) {
  const test = useMutation({
    mutationFn: () => unwrap(commands.testAiConnection(settings, keyText || null)),
    onSuccess: onTested,
    onError: (error) => onError(describeError(error)),
  });
  return (
    <div className="actions">
      <button className="btn" disabled={test.isPending} onClick={() => test.mutate()}>
        <Icon name="search" />{test.isPending ? t("Asking the server…") : t("Test connection")}
      </button>
    </div>
  );
}

function RunRow({ library, running, onError }: { library: LibraryDto; running: boolean; onError: (message: string) => void }) {
  const [started, setStarted] = useState<number | null>(null);
  const start = useMutation({
    mutationFn: () => unwrap(commands.startTagSuggestions(library.id, "")),
    onSuccess: setStarted,
    onError: (error) => onError(describeError(error)),
  });
  return (
    <div className="prow2">
      <div className="t">
        <b>{t("Models without tags in {name}", { name: library.name })}</b>
        <span>{started === null
          ? t("Asks about every model without own tags or suggestions. Locally this takes a few seconds per model, so start it on a folder first.")
          : plural(started, "Asking about {count} model. Progress is in the status bar.", "Asking about {count} models. Progress is in the status bar.")}</span>
      </div>
      <button className="btn" disabled={running || start.isPending} onClick={() => start.mutate()}><Icon name="tag" />{t("Suggest tags")}</button>
    </div>
  );
}

import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { describeError } from "../../ipc";
import { t } from "../../shared/i18n";
import { checksOnStart, setChecksOnStart, skippedVersion, skipVersion } from "./update-prefs";
import { checkForUpdate } from "./use-update";

/** Settings › Updates: the installed version, the start check, "Check now" and a skipped version. */
export function UpdateSettings() {
  const [version, setVersion] = useState("");
  const [onStart, setOnStart] = useState(checksOnStart);
  const [skipped, setSkipped] = useState(skippedVersion);
  const [result, setResult] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  useEffect(() => void getVersion().then(setVersion, () => setVersion("")), []);
  const check = () => {
    setChecking(true);
    checkForUpdate()
      .then((info) => {
        if (info && skipped === info.version) {
          skipVersion(null);
          setSkipped(null);
          void checkForUpdate();
        }
        setResult(info ? t("Version {version} is available. See the hint in the title bar.", { version: info.version }) : t("You have the newest version."));
      }, (error) => setResult(t("GitHub could not be reached: {reason}", { reason: describeError(error) })))
      .finally(() => setChecking(false));
  };
  return (
    <div className="panel">
      <div className="prow2">
        <div className="t">
          <b>{t("3D Hoard Deck {version}", { version })}</b>
          <span>{result ?? t("New versions come from the project's GitHub releases.")}</span>
        </div>
        <button className="btn" onClick={check} disabled={checking}>{checking ? t("Checking…") : t("Check now")}</button>
      </div>
      <label className="prow2">
        <div className="t"><b>{t("Check for updates on start")}</b><span>{t("Asks GitHub once, a few seconds after the app starts. Nothing else is sent.")}</span></div>
        <input type="checkbox" checked={onStart} onChange={(event) => { setChecksOnStart(event.target.checked); setOnStart(event.target.checked); }} />
      </label>
      {skipped && (
        <div className="prow2">
          <div className="t"><b>{t("Version {version} skipped", { version: skipped })}</b></div>
          <button className="btn ghost" onClick={() => { skipVersion(null); setSkipped(null); void checkForUpdate(); }}>{t("Show again")}</button>
        </div>
      )}
    </div>
  );
}

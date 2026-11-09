import { useState } from "react";
import { commands } from "../../ipc";
import { formatBytes } from "../../shared/format";
import { Icon } from "../../shared/icons";
import { t } from "../../shared/i18n";
import { skipVersion, whatsNew } from "./update-prefs";
import { hideUpdate, installUpdate, useUpdate } from "./use-update";

/**
 * A small pill in the title bar when a newer version is published; a click opens the card with what's new and
 * Update now / Later / Skip this version. Nothing pops up by itself.
 */
export function UpdatePill({ scanning }: { scanning: boolean }) {
  const update = useUpdate();
  const [open, setOpen] = useState(false);
  const info = update.info;
  if (!info || (update.hidden && update.phase === "idle")) return null;
  const busy = update.phase === "downloading" || update.phase === "restarting";
  return (
    <>
      <button
        className={`upd${busy ? " busy" : ""}${update.phase === "error" ? " err" : ""}`}
        onClick={() => setOpen(!open)}
        aria-expanded={open}
        aria-label={t("Version {version} is available", { version: info.version })}
      >
        <span className="dot" />{t("Update")} <span className="v">{info.version}</span>
      </button>
      {open && <UpdateCard scanning={scanning} onClose={() => setOpen(false)} />}
    </>
  );
}

/** The update card: under the pill, or `inline` on Settings › Updates. */
export function UpdateCard({ scanning, onClose, inline = false }: { scanning: boolean; onClose: () => void; inline?: boolean }) {
  const update = useUpdate();
  const [confirming, setConfirming] = useState(false);
  const info = update.info!;
  const notes = whatsNew(info.notes);
  const page = () => void commands.openReleasePage(info.version);
  const later = () => {
    hideUpdate();
    onClose();
  };
  const skip = () => {
    skipVersion(info.version);
    later();
  };
  const start = () => {
    if (scanning && !confirming) {
      setConfirming(true);
      return;
    }
    void installUpdate();
  };
  const failed = update.phase === "error";

  return (
    <div className={`upcard${inline ? " inline" : ""}`} role={inline ? "region" : "dialog"} aria-label={t("Update")}>
      <header>
        <span className={`ic${failed ? " err-ic" : ""}`}><Icon name={failed ? "close" : "refresh"} /></span>
        <div>
          <h4>{failed ? t("The update did not work") : t("Version {version} is available", { version: info.version })}</h4>
          <div className="sub">
            {t("You have {version}", { version: info.current })}{info.date ? ` · ${t("released {date}", { date: info.date })}` : ""}
          </div>
        </div>
        <button className="x" onClick={later} aria-label={t("Later")}><Icon name="close" /></button>
      </header>

      {!failed && update.phase === "idle" && (
        <div className="upnotes">
          <span className="lbl">{t("What's new")}</span>
          {notes.length > 0 && <ul>{notes.map((line) => <li key={line}>{line}</li>)}</ul>}
          <button className="linkish" onClick={page}>{t("All changes")}</button>
        </div>
      )}

      {update.phase === "downloading" && (
        <>
          <div className="prog"><i style={{ width: `${update.total > 0 ? Math.min(100, (update.done / update.total) * 100) : 5}%` }} /></div>
          <div className="progline">
            <span>{t("Downloading…")}</span>
            <span className="mono">{update.total > 0 ? `${formatBytes(update.done)} / ${formatBytes(update.total)}` : formatBytes(update.done)}</span>
          </div>
        </>
      )}
      {update.phase === "restarting" && <p className="hint">{t("Installing. The app restarts in a moment…")}</p>}
      {failed && <p className="hint err">{update.error}</p>}
      {confirming && update.phase === "idle" && <p className="hint">{t("A scan is running. It starts again after the restart; your index is safe.")}</p>}

      {!busy(update.phase) && (
        <div className="acts">
          {failed ? (
            <>
              {info.method === "install" && <button className="btn primary" onClick={() => void installUpdate()}>{t("Try again")}</button>}
              <button className="btn" onClick={page}>{t("Download from GitHub")}</button>
            </>
          ) : info.method === "install" ? (
            <button className="btn primary" onClick={start}>{confirming ? t("Update anyway") : t("Update now")}</button>
          ) : (
            <button className="btn primary" onClick={page}>{t("Open the release page")}</button>
          )}
          <button className="btn ghost" onClick={later}>{t("Later")}</button>
          <span className="spacer" />
          <button className="linkish" onClick={skip}>{t("Skip this version")}</button>
        </div>
      )}
    </div>
  );
}

const busy = (phase: string) => phase === "downloading" || phase === "restarting";

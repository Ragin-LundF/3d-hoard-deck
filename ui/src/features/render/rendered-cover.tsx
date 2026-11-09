import { useEffect } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { commands, describeError, unwrap, type ModelSummaryDto_Serialize } from "../../ipc";
import { t } from "../../shared/i18n";
import { showUndo } from "../batch/batch";
import { cancelRender, renderMode, requestRender, useRenderState } from "./render-queue";

export const renderedUrl = (id: number) => convertFileSrc(`rendered/${id}`, "hoard");

/** For a card without any picture: the rendered preview, or rendering it when it comes into view (or on request). */
export function RenderedCover({ model, fallback }: { model: ModelSummaryDto_Serialize; fallback: string }) {
  const state = useRenderState(model.id);
  const auto = renderMode() === "auto" && model.rendered === null;
  useEffect(() => {
    if (!auto) return;
    requestRender(model.library, model.id);
    return () => cancelRender(model.id);
  }, [auto, model.library, model.id]);
  if (model.rendered === "cached" || model.rendered === "kept") return <img className="render-img" src={renderedUrl(model.id)} alt="" loading="lazy" decoding="async" />;
  const render = (event: React.MouseEvent | React.KeyboardEvent) => {
    event.stopPropagation();
    requestRender(model.library, model.id, true);
  };
  const button = (label: string) => (
    <span className="render-btn" role="button" tabIndex={0} onClick={render} onKeyDown={(event) => (event.key === "Enter" || event.key === " ") && render(event)}>{label}</span>
  );
  return (
    <span className="ph render-ph">
      {fallback}
      {state === "busy" ? <span className="render-note">{t("Rendering a preview…")}</span>
        : state === "big" ? <span className="render-note">{t("Too big to render by itself")} · {button(t("Render"))}</span>
        : state === "error" ? <span className="render-note">{t("No preview possible")}</span>
        : model.rendered === "skipped" || renderMode() !== "auto" ? button(t("Render a preview")) : null}
    </span>
  );
}

type ActionsProps = { model: ModelSummaryDto_Serialize; canWrite: boolean; onMessage: (message: string) => void };

/** Over a card with an unsaved render: keep it (written into the folder, or kept in the index) or not this one. */
export function RenderedActions({ model, canWrite, onMessage }: ActionsProps) {
  const keep = () =>
    unwrap(commands.keepRenderedPreviews([model.id])).then(
      (kept) => {
        if (kept.kept > 0) showUndo(t("Preview kept"), () => unwrap(commands.resetRenderedPreviews([model.id], "cached")));
        else onMessage(t("Preview saved next to the file."));
      },
      (error) => onMessage(describeError(error)),
    );
  const skip = () =>
    unwrap(commands.skipRenderedPreviews([model.id])).then(
      () => showUndo(t("Preview not used"), () => unwrap(commands.resetRenderedPreviews([model.id], "cached"))),
      (error) => onMessage(describeError(error)),
    );
  return (
    <div className="render-actions">
      <span className="rendered-badge">{t("rendered")}</span>
      <button className="btn small primary" onClick={() => void keep()} title={canWrite ? t("Saved as preview_<file>.png in the model folder") : t("Kept in the app; nothing is written into this library")}>
        {canWrite ? t("Save") : t("Keep")}
      </button>
      <button className="btn small ghost" onClick={() => void skip()} aria-label={t("Not this one")} title={t("Not this one")}>✕</button>
    </div>
  );
}

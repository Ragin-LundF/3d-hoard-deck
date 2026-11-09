import { t } from "../../shared/i18n";

type Props = { what: string; saving: boolean; onSave: () => void; onDiscard: () => void };

/** Sticky at the bottom of a page with unsaved changes (tag rules, AI). */
export function SaveBar({ what, saving, onSave, onDiscard }: Props) {
  return (
    <div className="savebar" role="region" aria-label={t("Unsaved changes")}>
      <span className="d" />
      <span className="grow"><b>{what}</b> <span>{t("unsaved changes")}</span></span>
      <button className="btn ghost" onClick={onDiscard} disabled={saving}>{t("Discard")}</button>
      <button className="btn primary" onClick={onSave} disabled={saving}>{t("Save")}</button>
    </div>
  );
}

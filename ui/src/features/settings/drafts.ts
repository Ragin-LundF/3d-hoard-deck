import { useSyncExternalStore } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { t } from "../../shared/i18n";
import type { AiSettingsDto, TagRuleDto } from "../../ipc";

/** An unsaved edit that outlives its page, so switching settings pages keeps it; `null` means nothing unsaved. */
function draftStore<T>() {
  let value: T | null = null;
  const listeners = new Set<() => void>();
  const subscribe = (listener: () => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  };
  return {
    get: () => value,
    set: (next: T | null) => {
      value = next;
      listeners.forEach((listener) => listener());
    },
    use: () => useSyncExternalStore(subscribe, () => value),
  };
}

export const tagRuleDraft = draftStore<TagRuleDto[]>();
export const aiDraft = draftStore<{ settings: AiSettingsDto; key: string | null }>();

/** Unsaved tag rules or AI settings exist (kept across settings pages, lost on reload or close). */
export const hasUnsavedChanges = () => tagRuleDraft.get() !== null || aiDraft.get() !== null;

export function useHasUnsavedChanges(): boolean {
  // Both hooks always run: `a || b` would skip the second once there is a draft, and React then crashes.
  const rules = tagRuleDraft.use();
  const ai = aiDraft.use();
  return rules !== null || ai !== null;
}

/** Asks before something would throw unsaved changes away; true when there are none or the user agrees. */
export async function confirmDiscard(): Promise<boolean> {
  if (!hasUnsavedChanges()) return true;
  return ask(t("Tag rules or AI settings have unsaved changes. Discard them?"), {
    title: t("Unsaved changes"),
    kind: "warning",
    okLabel: t("Discard"),
    cancelLabel: t("Keep editing"),
  });
}

import { useSyncExternalStore } from "react";
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

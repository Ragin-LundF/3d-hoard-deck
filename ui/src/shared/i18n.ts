import { de } from "./i18n-de";

export type Language = "en" | "de";
export type LanguageChoice = Language | "system";

const STORAGE_KEY = "language";

export function languageChoice(): LanguageChoice {
  const stored = localStorage.getItem(STORAGE_KEY);
  return stored === "en" || stored === "de" ? stored : "system";
}

function resolve(choice: LanguageChoice): Language {
  if (choice !== "system") return choice;
  return navigator.language.toLowerCase().startsWith("de") ? "de" : "en";
}

/** Fixed for the session: switching reloads the window, so no component has to listen for it. */
export const language: Language = resolve(languageChoice());
document.documentElement.lang = language;

export function setLanguage(choice: LanguageChoice) {
  if (choice === "system") localStorage.removeItem(STORAGE_KEY);
  else localStorage.setItem(STORAGE_KEY, choice);
  location.reload();
}

/** A number in the app language: `1,234` in English, `1.234` in German; `digits` fixes the decimals. */
export function formatNumber(value: number, digits?: number): string {
  return value.toLocaleString(language, digits === undefined ? undefined : { minimumFractionDigits: digits, maximumFractionDigits: digits });
}

export function formatDate(date: Date): string {
  return date.toLocaleString(language, { dateStyle: "medium", timeStyle: "short" });
}

/** The English text is the key; `{name}` placeholders are filled from `values`. Missing translations stay English. */
export function t(text: string, values?: Record<string, string | number>): string {
  const template = language === "de" ? (de[text] ?? text) : text;
  if (!values) return template;
  return template.replace(/\{(\w+)\}/g, (match, key: string) => {
    const value = values[key];
    return value === undefined ? match : typeof value === "number" ? formatNumber(value) : value;
  });
}

/** `one` and `many` are English keys with a `{count}` placeholder. */
export function plural(count: number, one: string, many: string): string {
  return t(count === 1 ? one : many, { count });
}

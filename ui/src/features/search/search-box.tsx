import { useEffect, useRef, useState } from "react";
import { Icon } from "../../shared/icons";
import { t } from "../../shared/i18n";

const DEBOUNCE_MS = 150;
const isMac = navigator.userAgent.includes("Mac");

/** Typing is debounced; ⌘K / Ctrl K focuses the box, Escape clears it. */
export function SearchBox({ value, onSearch }: { value: string; onSearch: (text: string) => void }) {
  const [text, setText] = useState(value);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => setText(value), [value]);
  useEffect(() => {
    if (text === value) return;
    const timer = setTimeout(() => onSearch(text), DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [text, value, onSearch]);
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((isMac ? event.metaKey : event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        input.current?.focus();
        input.current?.select();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  return (
    <div className="search">
      <Icon name="search" />
      <input
        ref={input}
        type="search"
        value={text}
        placeholder={t("Search models… try  dragon type:3mf  tag:resin  is:favorite")}
        aria-label={t("Search library")}
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => event.key === "Escape" && setText("")}
      />
      <kbd>{isMac ? "⌘K" : "Ctrl K"}</kbd>
    </div>
  );
}

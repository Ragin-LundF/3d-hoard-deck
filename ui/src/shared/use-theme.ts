import { useEffect, useState } from "react";

type Theme = "dark" | "light";
const KEY = "hoarddeck.theme";

export function useTheme(): [Theme, () => void] {
  const [theme, setTheme] = useState<Theme>(() => (localStorage.getItem(KEY) === "light" ? "light" : "dark"));
  useEffect(() => {
    document.body.dataset.ui = theme;
    localStorage.setItem(KEY, theme);
  }, [theme]);
  return [theme, () => setTheme(theme === "dark" ? "light" : "dark")];
}

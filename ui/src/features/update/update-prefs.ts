/** Update preferences of this computer, kept in the webview's storage like the language. */
const CHECK_KEY = "updates.checkOnStart";
const SKIP_KEY = "updates.skipped";

export const checksOnStart = () => localStorage.getItem(CHECK_KEY) !== "false";
export const setChecksOnStart = (on: boolean) => localStorage.setItem(CHECK_KEY, String(on));
export const skippedVersion = () => localStorage.getItem(SKIP_KEY);
export const skipVersion = (version: string | null) =>
  version === null ? localStorage.removeItem(SKIP_KEY) : localStorage.setItem(SKIP_KEY, version);

/** Compares `1.10.0` and `1.9.2` by number; a release is newer than its own pre-release (`1.2.0` > `1.2.0-beta`). */
export function newer(version: string, than: string): boolean {
  const split = (text: string) => {
    const [numbers, pre] = text.replace(/^v/, "").split("+")[0]!.split(/-(.*)/);
    return { parts: numbers!.split(".").map((part) => Number(part) || 0), pre: pre ?? "" };
  };
  const [a, b] = [split(version), split(than)];
  for (let i = 0; i < Math.max(a.parts.length, b.parts.length); i++) {
    if ((a.parts[i] ?? 0) !== (b.parts[i] ?? 0)) return (a.parts[i] ?? 0) > (b.parts[i] ?? 0);
  }
  return a.pre === "" ? b.pre !== "" : b.pre !== "" && a.pre > b.pre;
}

/** A skipped version stays hidden until a newer one than it exists. */
export const hiddenBySkip = (version: string, skipped: string | null) => skipped !== null && !newer(version, skipped);

/** The first few list items of the release notes, without Markdown emphasis, for "What's new". */
export function whatsNew(notes: string, count = 3): string[] {
  return notes
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => /^[-*] /.test(line))
    .map((line) => line.replace(/^[-*] /, "").replace(/\*\*|`/g, "").split(/(?<=\.) /)[0]!)
    .slice(0, count);
}

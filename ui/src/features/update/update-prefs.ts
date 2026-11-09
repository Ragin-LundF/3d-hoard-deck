/** Update preferences of this computer, kept in the webview's storage like the language. */
const CHECK_KEY = "updates.checkOnStart";
const SKIP_KEY = "updates.skipped";

export const checksOnStart = () => localStorage.getItem(CHECK_KEY) !== "false";
export const setChecksOnStart = (on: boolean) => localStorage.setItem(CHECK_KEY, String(on));
export const skippedVersion = () => localStorage.getItem(SKIP_KEY);
export const skipVersion = (version: string | null) =>
  version === null ? localStorage.removeItem(SKIP_KEY) : localStorage.setItem(SKIP_KEY, version);

/** Compares `1.10.0` and `1.9.2` by number; a pre-release suffix is ignored. */
export function newer(version: string, than: string): boolean {
  const parts = (text: string) => text.replace(/^v/, "").split(/[-+]/)[0]!.split(".").map((part) => Number(part) || 0);
  const [a, b] = [parts(version), parts(than)];
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    if ((a[i] ?? 0) !== (b[i] ?? 0)) return (a[i] ?? 0) > (b[i] ?? 0);
  }
  return false;
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

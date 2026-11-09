/** Tag filters live in the normal search text (`tag:resin -tag:bust tag:32mm|28mm`), so they can be bookmarked and
 *  saved as smart collections. These helpers read and change just that part and keep the rest of the text. */

export type TagFilter = {
  /** Every one must be present. */
  include: string[];
  /** None may be present. */
  exclude: string[];
  /** At least one must be present (`tag:a|b`). */
  any: string[];
  /** Everything else in the search text. */
  rest: string[];
};

/** Tokens split at spaces outside quotes, quotes kept. */
function tokens(text: string): string[] {
  return text.match(/(?:[^\s"]+|"[^"]*")+/g) ?? [];
}

const unquote = (value: string) => value.replace(/"/g, "");
const quote = (tag: string) => (/\s/.test(tag) ? `"${tag}"` : tag);
const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

export function readTags(query: string): TagFilter {
  const filter: TagFilter = { include: [], exclude: [], any: [], rest: [] };
  for (const token of tokens(query)) {
    const lower = token.toLowerCase();
    if (lower.startsWith("-tag:")) filter.exclude.push(unquote(token.slice(5)));
    else if (lower.startsWith("tag:")) {
      const value = unquote(token.slice(4));
      if (value.includes("|")) filter.any.push(...value.split("|").filter(Boolean));
      else filter.include.push(value);
    } else filter.rest.push(token);
  }
  return filter;
}

export function writeTags(filter: TagFilter): string {
  const parts = [...filter.rest];
  if (filter.any.length > 0) parts.push(`tag:${quote(filter.any.join("|"))}`);
  parts.push(...filter.include.map((tag) => `tag:${quote(tag)}`), ...filter.exclude.map((tag) => `-tag:${quote(tag)}`));
  return parts.join(" ");
}

/** The tags the user asked for, whichever mode. */
export function wanted(filter: TagFilter): string[] {
  return [...filter.include, ...filter.any];
}

export type Mode = "all" | "any";

export function modeOf(filter: TagFilter): Mode {
  return filter.any.length > 0 ? "any" : "all";
}

/** Click on a chip: require the tag, or drop it again when it is already required or excluded. */
export function toggleInclude(query: string, tag: string): string {
  const filter = readTags(query);
  const active = wanted(filter).some((other) => same(other, tag)) || filter.exclude.some((other) => same(other, tag));
  const without = remove(filter, tag);
  if (active) return writeTags(without);
  return writeTags(modeOf(filter) === "any" ? { ...without, any: [...without.any, tag] } : { ...without, include: [...without.include, tag] });
}

/** ⌥-click: exclude the tag, or drop the exclusion. */
export function toggleExclude(query: string, tag: string): string {
  const filter = readTags(query);
  const excluded = filter.exclude.some((other) => same(other, tag));
  const without = remove(filter, tag);
  return writeTags(excluded ? without : { ...without, exclude: [...without.exclude, tag] });
}

/** "Match all" ↔ "Match any" for the required tags. */
export function setMode(query: string, mode: Mode): string {
  const filter = readTags(query);
  const tags = wanted(filter);
  return writeTags(mode === "any" && tags.length > 1 ? { ...filter, include: [], any: tags } : { ...filter, include: tags, any: [] });
}

function remove(filter: TagFilter, tag: string): TagFilter {
  const keep = (list: string[]) => list.filter((other) => !same(other, tag));
  const any = keep(filter.any);
  // A single "any" tag left is just a required one.
  return any.length === 1
    ? { ...filter, include: [...keep(filter.include), ...any], any: [], exclude: keep(filter.exclude) }
    : { ...filter, include: keep(filter.include), any, exclude: keep(filter.exclude) };
}

/** Tags that are probably the same, written differently: case, dashes or spaces, a plural "s". */
export function likelyDuplicates<T extends { name: string; count: number }>(tags: T[]): T[][] {
  const groups = new Map<string, T[]>();
  for (const tag of tags) {
    const key = normalize(tag.name);
    groups.set(key, [...(groups.get(key) ?? []), tag]);
  }
  return [...groups.values()]
    .filter((group) => group.length > 1)
    .map((group) => [...group].sort((a, b) => b.count - a.count));
}

function normalize(name: string): string {
  const plain = name.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");
  return plain.length > 3 && plain.endsWith("s") ? plain.slice(0, -1) : plain;
}

/**
 * Filters and orders items for the search box. Matching is case-insensitive on the name:
 * names that start with the query come first, then names with a word starting with it, then
 * names that contain it. Ties keep their original order. An empty query keeps everything.
 */
export function rank<T extends { name: string }>(items: readonly T[], query: string): T[] {
  const q = query.trim().toLowerCase();
  if (q === "") return [...items];
  const scored: Array<{ item: T; score: number; index: number }> = [];
  items.forEach((item, index) => {
    const name = item.name.toLowerCase();
    let score: number;
    if (name.startsWith(q)) score = 0;
    else if (name.split(/[\s\-_.]+/).some((w) => w.startsWith(q))) score = 1;
    else if (name.includes(q)) score = 2;
    else return;
    scored.push({ item, score, index });
  });
  scored.sort((a, b) => a.score - b.score || a.index - b.index);
  return scored.map((s) => s.item);
}

/** One argument per line; blank lines are dropped. `null` means "use the app's own". */
export function parseArgs(text: string): string[] | null {
  const args = text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l !== "");
  return args.length === 0 ? null : args;
}

/** Moves the item at `from` by `delta` places, clamped to the list. */
export function move<T>(items: readonly T[], from: number, delta: number): T[] {
  const to = Math.max(0, Math.min(items.length - 1, from + delta));
  const next = [...items];
  const [item] = next.splice(from, 1);
  if (item !== undefined) next.splice(to, 0, item);
  return next;
}

/** How many matches the `⌘O` list shows. */
export const MAX_SHOWN = 200;

/**
 * Ranks paths for a query typed in `⌘O`: a file name starting with the query first, then a
 * file name containing it, then a path containing it, then the query's letters in order. Each
 * space-separated word must match; shorter paths win a tie. An empty query keeps the order.
 */
export function rankFiles(files: readonly string[], query: string, limit = MAX_SHOWN): string[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return files.slice(0, limit);
  const scored: { path: string; score: number }[] = [];
  for (const path of files) {
    const lower = path.toLowerCase();
    const name = lower.slice(lower.lastIndexOf("/") + 1);
    let score = 0;
    for (const w of words) {
      const s = name.startsWith(w) ? 0 : name.includes(w) ? 1 : lower.includes(w) ? 2 : inOrder(lower, w) ? 3 : -1;
      if (s < 0) {
        score = -1;
        break;
      }
      score = Math.max(score, s);
    }
    if (score >= 0) scored.push({ path, score });
  }
  scored.sort((a, b) => a.score - b.score || a.path.length - b.path.length || a.path.localeCompare(b.path));
  return scored.slice(0, limit).map((s) => s.path);
}

function inOrder(text: string, letters: string): boolean {
  let i = 0;
  for (const c of text) if (c === letters[i] && ++i === letters.length) return true;
  return false;
}

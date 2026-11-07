/** One terminal's buffer, as rows of text and whether a row continues the one above. */
export interface Rows {
  readonly length: number;
  row(i: number): { text: string; wrapped: boolean } | undefined;
}

export interface FindHit {
  paneId: string;
  /** The logical line's first and last buffer rows, for REVEAL_ROWS_EVENT. */
  rows: [number, number];
  text: string;
  /** Where the match starts in `text`. */
  at: number;
}

export const FIND_ALL_LIMIT = 200;

/** A pane's logical lines (wrapped rows joined), read once and searched on every key. */
export interface PaneLines {
  paneId: string;
  lines: { text: string; lower: string; rows: [number, number] }[];
}

/** Reads each pane's buffer into logical lines; the slow part, done once per sheet. */
export function readLines(panes: { paneId: string; rows: Rows }[]): PaneLines[] {
  return panes.map(({ paneId, rows }) => {
    const lines: PaneLines["lines"] = [];
    let i = 0;
    while (i < rows.length) {
      let text = rows.row(i)?.text ?? "";
      let end = i;
      for (let next = rows.row(end + 1); next?.wrapped; next = rows.row(end + 1)) {
        text += next.text;
        end++;
      }
      lines.push({ text, lower: text.toLowerCase(), rows: [i, end] });
      i = end + 1;
    }
    return { paneId, lines };
  });
}

/**
 * Case-insensitive matches of `query` in lines read by `readLines` (task 8.46), one hit per
 * logical line, newest output last within a pane. Stops at `limit` hits in all.
 */
export function findInLines(panes: PaneLines[], query: string, limit = FIND_ALL_LIMIT): { hits: FindHit[]; more: boolean } {
  const needle = query.toLowerCase();
  const hits: FindHit[] = [];
  if (!needle.trim()) return { hits, more: false };
  for (const { paneId, lines } of panes) {
    for (const line of lines) {
      const at = line.lower.indexOf(needle);
      if (at < 0) continue;
      if (hits.length === limit) return { hits, more: true };
      hits.push({ paneId, rows: line.rows, text: line.text.trimEnd(), at });
    }
  }
  return { hits, more: false };
}

/** `readLines` and `findInLines` in one step. */
export function findAll(panes: { paneId: string; rows: Rows }[], query: string, limit = FIND_ALL_LIMIT): { hits: FindHit[]; more: boolean } {
  return findInLines(readLines(panes), query, limit);
}

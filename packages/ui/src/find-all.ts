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

/**
 * Case-insensitive matches of `query` in each pane's buffer (task 8.46), one hit per logical line
 * (wrapped rows joined), newest output last within a pane. Stops at `limit` hits in all.
 */
export function findAll(panes: { paneId: string; rows: Rows }[], query: string, limit = FIND_ALL_LIMIT): { hits: FindHit[]; more: boolean } {
  const needle = query.toLowerCase();
  const hits: FindHit[] = [];
  if (!needle.trim()) return { hits, more: false };
  for (const { paneId, rows } of panes) {
    let i = 0;
    while (i < rows.length) {
      const first = rows.row(i);
      let text = first?.text ?? "";
      let end = i;
      for (let next = rows.row(end + 1); next?.wrapped; next = rows.row(end + 1)) {
        text += next.text;
        end++;
      }
      const at = text.toLowerCase().indexOf(needle);
      if (at >= 0) {
        if (hits.length === limit) return { hits, more: true };
        hits.push({ paneId, rows: [i, end], text: text.trimEnd(), at });
      }
      i = end + 1;
    }
  }
  return { hits, more: false };
}

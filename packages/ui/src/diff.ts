import type { Hunk, ToolEdit } from "@terminescu/host";

/** One diff row: kept, removed or added. */
export interface DiffLine {
  sign: " " | "-" | "+";
  text: string;
}

/** Context lines kept around a change computed from an edit. */
const CONTEXT = 2;

const linesOf = (s: string) => {
  if (s === "") return [];
  const out = s.split("\n");
  if (out[out.length - 1] === "") out.pop();
  return out;
};

/**
 * The rows of one Edit or Write, for when Claude Code stored no patch (a denied or failed call):
 * lines both sides share at the start and end fold to two lines of context.
 */
export function editDiff(edit: ToolEdit): DiffLine[] {
  const a = linesOf(edit.old);
  const b = linesOf(edit.new);
  let head = 0;
  while (head < a.length && head < b.length && a[head] === b[head]) head++;
  let tail = 0;
  while (tail < a.length - head && tail < b.length - head && a[a.length - 1 - tail] === b[b.length - 1 - tail]) tail++;
  const keep = (text: string): DiffLine => ({ sign: " ", text });
  return [
    ...a.slice(Math.max(0, head - CONTEXT), head).map(keep),
    ...a.slice(head, a.length - tail).map((text): DiffLine => ({ sign: "-", text })),
    ...b.slice(head, b.length - tail).map((text): DiffLine => ({ sign: "+", text })),
    ...a.slice(a.length - tail, a.length - tail + CONTEXT).map(keep),
  ];
}

/** The rows of a stored patch. "\ No newline" notes read as context. */
export function hunkDiff(hunk: Hunk): DiffLine[] {
  return hunk.lines.map((l) => {
    const sign = l[0];
    return sign === "-" || sign === "+" ? { sign, text: l.slice(1) } : { sign: " ", text: sign === " " ? l.slice(1) : l };
  });
}

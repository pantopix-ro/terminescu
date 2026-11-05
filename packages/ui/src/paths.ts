/** A file path found in a line of terminal output; `start` and `end` are string offsets. */
export interface FoundPath {
  start: number;
  end: number;
  /** The path as printed, without a trailing `:line:col`. */
  path: string;
}

// Allowed in a path token: no spaces, quotes or brackets.
const TOKEN = /[^\s'"`()<>[\]{}|]+/g;
const SHAPE = /^(?:~\/|\.{1,2}\/|\/)?[\w.@+-]+(?:\/[\w.@+-]+)*\/?$/;
const EXT = /\.[A-Za-z][A-Za-z0-9]{0,9}$/;

/**
 * File paths in a line: tokens with a `/` or a file extension, such as `src/main.rs:12:4`,
 * `./README.md` or `~/notes.md`. URLs, numbers like `1.2.3` and `1/2` are left out; whether
 * the file exists is the core's call.
 */
export function findPaths(line: string): FoundPath[] {
  const out: FoundPath[] = [];
  for (const m of line.matchAll(TOKEN)) {
    let text = m[0];
    let start = m.index;
    if (text.includes("://")) continue;
    // `file.ts:12:4`, `file.ts:12`, then sentence punctuation.
    text = text.replace(/[.,;:!?]+$/, "");
    const lineCol = /(:\d+){1,2}$/.exec(text);
    const path = lineCol ? text.slice(0, lineCol.index) : text;
    const lead = /^[^~./\w@+-]+/.exec(path)?.[0].length ?? 0;
    const p = path.slice(lead);
    start += lead;
    if (!p || !SHAPE.test(p) || !/[A-Za-z]/.test(p)) continue;
    const name = p.replace(/\/$/, "").split("/").pop() ?? "";
    if (!p.includes("/") && !EXT.test(name)) continue;
    if (/^\.+$/.test(name)) continue;
    out.push({ start, end: start + text.length - lead, path: p });
  }
  return out;
}

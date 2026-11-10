// Every chord the README and the docs name is one the app or a plugin has by default
// (add-configurable-shortcuts 3.3), so a moved default never leaves a stale line behind.

import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { ACTIONS, RESERVED, chordLabel } from "../src/keys";
import { DEFAULT_SETTINGS } from "../src/protocol";

const root = join(__dirname, "../../..");
const read = (p: string) => readFileSync(join(root, p), "utf8");

/** Chords that belong to other programs or to one field, named in the docs on purpose. */
const ELSEWHERE = ["⌃G"];

/** One spelling: the modifiers as a sorted set, then the key. */
const norm = (chord: string) => {
  const mods = [...chord.matchAll(/[⌃⌥⇧⌘]/g)].map((m) => m[0]).sort().join("");
  return mods + chord.replace(/[⌃⌥⇧⌘]/g, "");
};

/** Each plugin action's default chord, read from the plugins' sources. */
function pluginChords(): string[] {
  const out: string[] = [];
  for (const pkg of ["plugin-git", "plugin-claude"]) {
    const dir = join(root, "packages", pkg, "src");
    for (const f of readdirSync(dir).filter((n) => /\.tsx?$/.test(n)))
      for (const m of readFileSync(join(dir, f), "utf8").matchAll(/accel: "([^"]+)"/g)) out.push(chordLabel(m[1]!));
  }
  return out;
}

describe("chords in the docs", () => {
  it("are defaults", () => {
    const known = new Set(
      [
        ...ACTIONS.map((a) => chordLabel(a.accel)),
        ...RESERVED.map((r) => chordLabel(r.accel)),
        ...Object.values(DEFAULT_SETTINGS.terminalKeys).filter(Boolean).map(chordLabel),
        ...pluginChords(),
        ...ELSEWHERE,
      ].map(norm),
    );
    const named = ["README.md", "docs/getting-started.md", "docs/plugins.md"].flatMap((p) =>
      [...read(p).matchAll(/[⌃⌥⇧⌘]+(?:[A-Z0-9↵←→↑↓\\=-]|F\d+)/g)].map((m) => ({ p, chord: m[0] })),
    );
    expect(named.length).toBeGreaterThan(5);
    expect(named.filter((n) => !known.has(norm(n.chord)))).toEqual([]);
  });
});

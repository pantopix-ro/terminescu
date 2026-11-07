// xterm.js with the Unicode 11 table gives each character the width the daemon's grid gives it
// (task 8.41), so a snapshot replays on the same columns. `crates/ptyd/tests/widths.rs` checks
// the same fixture against the daemon. Also: a synchronized-output block (DEC 2026) split
// across two writes keeps the mode on until its end.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { Terminal } from "@xterm/headless";
import { describe, expect, it } from "vitest";

const fixture = JSON.parse(readFileSync(fileURLToPath(new URL("../../../crates/ptyd/tests/fixtures/widths.json", import.meta.url)), "utf8")) as {
  cases: { name: string; text: string; width: number }[];
};

function terminal(): Terminal {
  const t = new Terminal({ cols: 40, rows: 4, allowProposedApi: true });
  t.loadAddon(new Unicode11Addon() as never);
  t.unicode.activeVersion = "11";
  return t;
}

const write = (t: Terminal, data: string) => new Promise<void>((done) => t.write(data, done));

describe("Unicode 11 widths", () => {
  for (const c of fixture.cases) {
    it(c.name, async () => {
      const t = terminal();
      await write(t, c.text);
      expect(t.buffer.active.cursorX).toBe(c.width);
    });
  }

  it("the default table would get the newer emoji wrong", async () => {
    const t = new Terminal({ cols: 40, rows: 4, allowProposedApi: true });
    await write(t, "\u{1F970}");
    expect(t.buffer.active.cursorX).toBe(1);
  });
});

describe("synchronized output", () => {
  it("a DEC 2026 block split over two writes stays on until its end", async () => {
    const t = terminal();
    await write(t, "\x1b[?2026hfirst half");
    expect(t.modes.synchronizedOutputMode).toBe(true);
    await write(t, " second half\x1b[?2026l");
    expect(t.modes.synchronizedOutputMode).toBe(false);
    expect(t.buffer.active.getLine(0)!.translateToString(true)).toBe("first half second half");
  });
});

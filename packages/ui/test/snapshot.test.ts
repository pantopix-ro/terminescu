// The daemon rebuilds a pane's screen in xterm.js from a snapshot (D31). This test replays the
// spike 0.1 stream into one headless terminal and the daemon's committed snapshot of the same
// stream into another, then checks that both show the same thing.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { Terminal, type IBuffer } from "@xterm/headless";
import { describe, expect, it } from "vitest";

const dir = fileURLToPath(new URL("../../../crates/ptyd/tests/fixtures/reattach/", import.meta.url));
const stream = readFileSync(dir + "claude-session.bin");
const snapshot = readFileSync(dir + "claude-session.snapshot.bin");
const events = JSON.parse(readFileSync(dir + "claude-session.events.json", "utf8")) as {
  offset: number;
  cols: number;
  rows: number;
}[];

function terminal(cols: number, rows: number): Terminal {
  return new Terminal({ cols, rows, scrollback: 10_000, allowProposedApi: true });
}

function write(t: Terminal, data: Uint8Array): Promise<void> {
  return new Promise((resolve) => t.write(data, resolve));
}

function lines(b: IBuffer): string[] {
  const out: string[] = [];
  for (let i = 0; i < b.length; i++) {
    const line = b.getLine(i)!;
    out.push(`${line.isWrapped ? "+" : " "}${line.translateToString(true)}`);
  }
  // Blank lines below the last written one depend on how the screen was filled, not on content.
  while (out.length && out[out.length - 1] === " ") out.pop();
  return out;
}

function styles(t: Terminal): string[] {
  const b = t.buffer.active;
  const out: string[] = [];
  for (let y = 0; y < t.rows; y++) {
    const line = b.getLine(b.viewportY + y)!;
    for (let x = 0; x < t.cols; x++) {
      const c = line.getCell(x)!;
      // A written space and a cell never written look the same.
      const ch = c.getChars() || " ";
      if (ch === " " && c.isAttributeDefault()) continue;
      out.push(
        `${y},${x} ${ch} fg${c.getFgColorMode()}:${c.getFgColor()} bg${c.getBgColorMode()}:${c.getBgColor()} ` +
          `b${c.isBold()}d${c.isDim()}i${c.isItalic()}u${c.isUnderline()}v${c.isInverse()}`,
      );
    }
  }
  return out;
}

function state(t: Terminal) {
  const b = t.buffer;
  return {
    active: b.active.type,
    cursor: [b.active.cursorX, b.active.cursorY],
    modes: t.modes,
    normal: lines(b.normal),
    alternate: b.active.type === "alternate" ? lines(b.alternate) : [],
    styles: styles(t),
  };
}

describe("daemon snapshot in xterm.js", () => {
  it("matches the raw stream of a Claude session", async () => {
    const raw = terminal(events[0]!.cols, events[0]!.rows);
    let at = 0;
    for (const e of events.slice(1)) {
      await write(raw, stream.subarray(at, e.offset));
      raw.resize(e.cols, e.rows);
      at = e.offset;
    }
    await write(raw, stream.subarray(at));

    const last = events[events.length - 1]!;
    const restored = terminal(last.cols, last.rows);
    await write(restored, snapshot);

    const want = state(raw);
    const got = state(restored);
    expect(got.active).toBe("alternate");
    expect(got.alternate).toEqual(want.alternate);
    expect(got.styles).toEqual(want.styles);
    expect(got.cursor).toEqual(want.cursor);
    expect(got.modes).toEqual(want.modes);
    expect(got.normal).toEqual(want.normal);
  });
});

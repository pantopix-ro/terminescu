// Prompt marks from OSC 133 (task 8.45) on a headless terminal: jumps step through prompts,
// a scroll in between starts afresh, and the last finished command's output gets selected.
import { Terminal } from "@xterm/headless";
import { describe, expect, it } from "vitest";
import { attachPromptMarks, type MarkTerminal } from "../src/prompt-marks";

const write = (t: Terminal, data: string) => new Promise<void>((done) => t.write(data, done));
const A = "\x1b]133;A;src=terminescu\x07";
const C = "\x1b]133;C;src=terminescu\x07";
const D = (code: number) => `\x1b]133;D;${code};src=terminescu\x07`;
const lines = (n: number, tag: string) => Array.from({ length: n }, (_, i) => `${tag} ${i}\r\n`).join("");

async function setup() {
  const t = new Terminal({ cols: 40, rows: 5, allowProposedApi: true });
  const selected: [number, number][] = [];
  const term = Object.assign(t, { selectLines: (a: number, b: number) => selected.push([a, b]) }) as unknown as MarkTerminal;
  const marks = attachPromptMarks(term, { tick: "#888", failed: "#f00" });
  // Three commands of 10 lines each, then a prompt being typed at.
  for (const [i, code] of [0, 1, 0].entries()) await write(t, `${A}$ cmd${i}\r\n${C}${lines(10, `out${i}`)}${D(code)}`);
  await write(t, `${A}$ `);
  return { t, marks, selected };
}

describe("prompt marks", () => {
  it("records one mark per prompt", async () => {
    const { marks } = await setup();
    expect(marks.rows()).toEqual([0, 11, 22, 33]);
  });

  it("⌘↑ steps back through prompts and ⌘↓ forward to the bottom", async () => {
    const { t, marks } = await setup();
    expect(marks.jump(true)).toBe(true);
    expect(t.buffer.active.viewportY).toBe(22);
    marks.jump(true);
    expect(t.buffer.active.viewportY).toBe(11);
    marks.jump(true);
    expect(t.buffer.active.viewportY).toBe(0);
    expect(marks.jump(true)).toBe(false);
    marks.jump(false);
    expect(t.buffer.active.viewportY).toBe(11);
    marks.jump(false);
    marks.jump(false);
    expect(t.buffer.active.viewportY).toBe(t.buffer.active.baseY);
  });

  it("selects the output of the prompt jumped to, else the last finished one", async () => {
    const { marks, selected } = await setup();
    expect(marks.selectOutput()).toBe(true);
    expect(selected.pop()).toEqual([23, 32]);
    marks.jump(true);
    marks.jump(true);
    marks.selectOutput();
    expect(selected.pop()).toEqual([12, 21]);
  });

  it("ignores untagged marks once tagged ones arrived, and the alternate screen", async () => {
    const { t, marks } = await setup();
    await write(t, "\r\n\x1b]133;A\x07nested\r\n\x1b[?1049h" + A + "\x1b[?1049l");
    expect(marks.rows()).toEqual([0, 11, 22, 33]);
  });
});

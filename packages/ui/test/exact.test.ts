import { describe, expect, it } from "vitest";
import { eolOf, exactState, separatorOf, textOf } from "../src/text";

describe("byte-exact text (task 7.1, D29)", () => {
  it("edits one word of a CRLF file with trailing spaces and changes one line", () => {
    const file = "# Notes  \r\nfirst line \r\nsecond line\t\r\n\r\nlast line   \r\n";
    let state = exactState(file);
    // Editor positions count a CRLF as one; a string offset into the file would be off.
    const at = state.doc.line(3).from;
    state = state.update({ changes: { from: at, to: at + "second".length, insert: "other" } }).state;
    const saved = textOf(state);
    const before = file.split("\r\n");
    const after = saved.split("\r\n");
    expect(after).toHaveLength(before.length);
    const changed = before.flatMap((line, i) => (line === after[i] ? [] : [i]));
    expect(changed).toEqual([2]);
    expect(after[2]).toBe("other line\t");
    expect(saved).not.toMatch(/[^\r]\n/);
  });

  it("keeps a mixed file's endings and a lone CR as they are", () => {
    const file = "a\r\nb\nc\rd\n";
    expect(eolOf(file)).toBe("Mixed");
    expect(separatorOf(file)).toBe("\n");
    expect(textOf(exactState(file))).toBe(file);
  });

  it("reads LF, CRLF and no line break", () => {
    expect(eolOf("a\nb\n")).toBe("LF");
    expect(eolOf("a\r\nb\r\n")).toBe("CRLF");
    expect(eolOf("one line")).toBe("LF");
    expect(textOf(exactState("a\r\nb"))).toBe("a\r\nb");
  });
});

describe("pasted line breaks (D29)", () => {
  it("take the file's own endings, and a Mixed file takes them as they come", async () => {
    const { fitBreaks } = await import("../src/text");
    const crlf = exactState("a\r\nb\r\n");
    const pasted = fitBreaks("x\ny\r\nz", crlf);
    expect(pasted).toBe("x\r\ny\r\nz");
    const after = crlf.update({ changes: { from: 1, insert: crlf.toText(pasted) } }).state;
    expect(eolOf(textOf(after))).toBe("CRLF");
    expect(fitBreaks("x\r\ny", exactState("a\nb\n"))).toBe("x\ny");
    expect(fitBreaks("x\r\ny", exactState("a\r\nb\nc"))).toBe("x\r\ny");
  });
});

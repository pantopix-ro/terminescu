import { describe, expect, it } from "vitest";
import { editDiff, hunkDiff } from "../src/diff";

const rows = (d: { sign: string; text: string }[]) => d.map((l) => l.sign + l.text);

describe("editDiff", () => {
  it("folds shared lines to two of context each side", () => {
    const old = "a\nb\nc\nd\nold\ne\nf\ng\n";
    const now = "a\nb\nc\nd\nnew\nmore\ne\nf\ng\n";
    expect(rows(editDiff({ path: "x", old, new: now }))).toEqual([" c", " d", "-old", "+new", "+more", " e", " f"]);
  });

  it("shows a Write as added lines", () => {
    expect(rows(editDiff({ path: "x", old: "", new: "# Notes\n\nPROJ-477\n" }))).toEqual(["+# Notes", "+", "+PROJ-477"]);
  });

  it("shows a pure removal", () => {
    expect(rows(editDiff({ path: "x", old: "keep\ndrop", new: "keep" }))).toEqual([" keep", "-drop"]);
  });
});

describe("hunkDiff", () => {
  it("splits the sign from the text", () => {
    expect(hunkDiff({ oldStart: 1, newStart: 1, lines: ["-hi", "+hello", " ctx", "\\ No newline at end of file"] })).toEqual([
      { sign: "-", text: "hi" },
      { sign: "+", text: "hello" },
      { sign: " ", text: "ctx" },
      { sign: " ", text: "\\ No newline at end of file" },
    ]);
  });
});

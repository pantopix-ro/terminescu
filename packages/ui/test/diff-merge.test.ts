import { getOriginalDoc, unifiedMergeView } from "@codemirror/merge";
import { EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { asOriginal, baseUpdate, middleChange } from "../src/diff-merge";

describe("diff tab in place (polish-git-plugin D4)", () => {
  it("replaces only the middle both texts do not share", () => {
    expect(middleChange("abc", "abc")).toBeUndefined();
    expect(middleChange("a\nb\nc\n", "a\nB\nc\n")).toEqual({ from: 2, to: 3, insert: "B" });
    expect(middleChange("aaa", "aaaa")).toEqual({ from: 3, to: 3, insert: "a" });
    expect(middleChange("abc", "")).toEqual({ from: 0, to: 3, insert: "" });
    expect(middleChange("", "x")).toEqual({ from: 0, to: 0, insert: "x" });
  });

  it("moves a unified view's base to the new text", () => {
    const base = "one\ntwo\nthree\n";
    const state = EditorState.create({ doc: "one\n2\nthree\n", extensions: unifiedMergeView({ original: base, mergeControls: false }) });
    expect(baseUpdate(base, base)).toBeUndefined();
    const next = "one\ntwo\nthree\nfour\n";
    const moved = state.update({ effects: baseUpdate(base, next)! }).state;
    expect(getOriginalDoc(moved).toString()).toBe(next);
    expect(moved.doc.toString()).toBe("one\n2\nthree\n");
  });

  it("moves a unified view's CRLF base without a length mismatch", () => {
    const base = "one\r\ntwo\r\nthree\r\n";
    const state = EditorState.create({ doc: "one\n2\nthree\n", extensions: unifiedMergeView({ original: base, mergeControls: false }) });
    const next = "one\r\ntwo\r\nthree\r\nfour\r\n";
    const moved = state.update({ effects: baseUpdate(base, next)! }).state;
    expect(getOriginalDoc(moved).toString()).toBe("one\ntwo\nthree\nfour\n");
    expect(baseUpdate(base, base.replace(/\r\n/g, "\n"))).toBeUndefined();
  });

  it("moves a side-by-side CRLF base in place", () => {
    // The left editor holds the base as the merge view gives it: carriage returns dropped.
    let a = EditorState.create({ doc: "one\r\ntwo\r\n" });
    expect(a.doc.toString()).toBe("one\ntwo\n");
    const text = asOriginal("one\r\nTWO\r\n");
    const change = middleChange(a.doc.toString(), text)!;
    a = a.update({ changes: change }).state;
    expect(a.doc.toString()).toBe("one\nTWO\n");
    expect(middleChange(a.doc.toString(), asOriginal("one\r\nTWO\r\n"))).toBeUndefined();
  });
});

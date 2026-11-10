import { getOriginalDoc, unifiedMergeView } from "@codemirror/merge";
import { EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { baseUpdate, middleChange } from "../src/diff-merge";

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
});

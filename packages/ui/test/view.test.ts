import type { Layout } from "@terminescu/host";
import { describe, expect, it } from "vitest";
import { composeView, fitView, hideRow, rowsOf, showBeside } from "../src/view";

// Task 8.71: the rows a window shows side by side.

const leaf = (id: string): Layout => ({ type: "pane", id });
const pair: Layout = { type: "split", id: "view-1", dir: "row", children: [leaf("a"), leaf("b")], sizes: [500, 500] };

describe("the window's view", () => {
  it("puts a row beside another, and moves a shown row instead of showing it twice", () => {
    const v = showBeside(leaf("a"), "a", "b", "right", "view-1");
    expect(v).toEqual(pair);
    const moved = showBeside(v, "b", "a", "down", "view-2")!;
    expect(rowsOf(moved)).toEqual(["b", "a"]);
    expect(moved).toMatchObject({ dir: "column" });
    expect(showBeside(v, "a", "a", "left", "view-3")).toBeUndefined();
    expect(showBeside(v, "c", "d", "left", "view-3")).toBeUndefined();
    // An area under twice the smallest share cannot halve.
    const thin: Layout = { ...pair, sizes: [920, 80] };
    expect(showBeside(thin, "b", "c", "right", "view-4")).toBeUndefined();
  });

  it("drops rows that left the window, and swaps the selected row for the one before", () => {
    expect(fitView(pair, ["a"], "a", "a")).toEqual(leaf("a"));
    expect(rowsOf(fitView(pair, ["a", "b", "c"], "c", "b"))).toEqual(["a", "c"]);
    expect(rowsOf(fitView(pair, ["a", "b", "c"], "c", undefined))).toEqual(["c", "b"]);
    expect(fitView(undefined, ["a"], "a", undefined)).toEqual(leaf("a"));
    expect(fitView(pair, ["a", "b"], "b", "a")).toBe(pair);
    expect(hideRow(leaf("a"), "a")).toBeUndefined();
  });

  it("joins the rows' layouts into one tree, skipping a row with none", () => {
    const tab: Layout = { type: "split", id: "s", dir: "column", children: [leaf("p1"), leaf("p2")], sizes: [500, 500] };
    const all = composeView(pair, (row) => (row === "a" ? tab : leaf("p3")));
    expect(all).toEqual({ ...pair, children: [tab, leaf("p3")] });
    expect(composeView(pair, (row) => (row === "a" ? tab : undefined))).toEqual(tab);
  });
});

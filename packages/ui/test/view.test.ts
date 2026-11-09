import type { Layout } from "@terminescu/host";
import { describe, expect, it } from "vitest";
import { composeView, fitView, folderLines, hideRow, rowsOf, showBeside } from "../src/view";

// Tasks 8.71 and 8.75: the tabs a window shows side by side.

const leaf = (id: string): Layout => ({ type: "pane", id });
const pair: Layout = { type: "split", id: "view-1", dir: "row", children: [leaf("a"), leaf("b")], sizes: [500, 500] };

describe("the window's view", () => {
  it("puts a tab beside another, and moves a shown tab instead of showing it twice", () => {
    const v = showBeside(leaf("a"), "a", ["b"], "right", "view-1");
    expect(v).toEqual(pair);
    const moved = showBeside(v, "b", ["a"], "down", "view-2")!;
    expect(rowsOf(moved)).toEqual(["b", "a"]);
    expect(moved).toMatchObject({ dir: "column" });
    expect(showBeside(v, "a", ["a"], "left", "view-3")).toBeUndefined();
    expect(showBeside(v, "c", ["d"], "left", "view-3")).toBeUndefined();
    // An area under twice the smallest share cannot halve.
    const thin: Layout = { ...pair, sizes: [920, 80] };
    expect(showBeside(thin, "b", ["c"], "right", "view-4")).toBeUndefined();
  });

  it("puts a folder's tabs beside a tab, sharing half its area", () => {
    const v = showBeside(leaf("a"), "a", ["b", "c"], "right", "view-1")!;
    expect(rowsOf(v)).toEqual(["a", "b", "c"]);
    expect(v).toMatchObject({ dir: "row", sizes: [500, 250, 250] });
    // A folder holding the target cannot go beside it.
    expect(showBeside(v, "b", ["b", "c"], "left", "view-2")).toBeUndefined();
    const down = showBeside(leaf("a"), "a", ["b", "c"], "down", "view-3")!;
    expect(down).toMatchObject({ dir: "column" });
  });

  it("drops rows that left the window, and swaps the selected row for the one before", () => {
    expect(fitView(pair, ["a"], "a", "a")).toEqual(leaf("a"));
    expect(rowsOf(fitView(pair, ["a", "b", "c"], "c", "b"))).toEqual(["a", "c"]);
    expect(rowsOf(fitView(pair, ["a", "b", "c"], "c", undefined))).toEqual(["c", "b"]);
    expect(fitView(undefined, ["a"], "a", undefined)).toEqual(leaf("a"));
    expect(fitView(pair, ["a", "b"], "b", "a")).toBe(pair);
    // The selected tab closed: the tab selected next takes its place.
    expect(rowsOf(fitView(pair, ["a", "c"], "c", "b"))).toEqual(["a", "c"]);
    expect(fitView(leaf("b"), ["c"], "c", "b")).toEqual(leaf("c"));
    expect(hideRow(leaf("a"), "a")).toBeUndefined();
  });

  it("joins the rows' layouts into one tree, skipping a row with none", () => {
    const tab: Layout = { type: "split", id: "s", dir: "column", children: [leaf("p1"), leaf("p2")], sizes: [500, 500] };
    const all = composeView(pair, (row) => (row === "a" ? tab : leaf("p3")));
    expect(all).toEqual({ ...pair, children: [tab, leaf("p3")] });
    expect(composeView(pair, (row) => (row === "a" ? tab : undefined))).toEqual(tab);
  });

  it("lists a folder's tabs with their directories, the shown one marked", () => {
    const lines = folderLines([{ id: "t1" }, { id: "t2" }], "t2", () => "zsh", { t1: "~/work/api", t2: "~/work/web · npm" });
    expect(lines).toEqual(["  zsh  ~/work/api", "▸ zsh  ~/work/web · npm"]);
    expect(folderLines([{ id: "t1" }], undefined, () => "zsh", {})).toEqual(["  zsh"]);
  });
});

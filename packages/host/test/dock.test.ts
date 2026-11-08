import { describe, expect, it } from "vitest";
import { dockPlace, panesOf, splitAt, TOTAL } from "../src/layout";
import { createMockHost } from "../src/mock";
import type { Layout, Workspace } from "../src/protocol";

// Task 8.32: a tab dropped on a pane's side joins that tab, and a pane can leave for a tab.

const pane = (id: string): Layout => ({ type: "pane", id });

describe("splitAt", () => {
  it("puts a tab before or after a pane, in the side's direction", () => {
    for (const [side, dir, order] of [
      ["left", "row", ["b", "a"]],
      ["right", "row", ["a", "b"]],
      ["up", "column", ["b", "a"]],
      ["down", "column", ["a", "b"]],
    ] as const) {
      const [d, before] = dockPlace(side);
      const out = splitAt(pane("a"), "a", d, before, pane("b"), "s") as Layout;
      expect(out.type === "split" && out.dir).toBe(dir);
      expect(panesOf(out)).toEqual(order);
    }
  });

  it("merges a split tab into a parent of its direction and refuses shares too small", () => {
    const tab: Layout = { type: "split", id: "s1", dir: "row", children: [pane("b"), pane("c")], sizes: [500, 500] };
    const out = splitAt(pane("a"), "a", "row", false, tab, "s2") as Layout;
    expect(out.type === "split" && out.children.length).toBe(3);
    expect(out.type === "split" && out.sizes.reduce((x, y) => x + y)).toBe(TOTAL);
    const lopsided: Layout = { type: "split", id: "s3", dir: "row", children: [pane("b"), pane("c")], sizes: [940, 60] };
    expect(splitAt(pane("a"), "a", "row", true, lopsided, "s4")).toBe("small");
    expect(splitAt(pane("a"), "zz", "row", true, pane("b"), "s5")).toBeUndefined();
  });
});

describe("mock dock commands", () => {
  const seed = (): Workspace => ({
    windows: [
      {
        id: "w1",
        sideTabs: [
          {
            id: "s1",
            title: null,
            tabs: [
              { id: "t1", title: null, layout: pane("p1") },
              { id: "t2", title: null, layout: pane("p2") },
            ],
          },
        ],
      },
    ],
  });

  it("docks a tab and refuses its own pane", async () => {
    const host = createMockHost(seed());
    await expect(host.send({ type: "DockTab", tabId: "t1", paneId: "p1", side: "left" })).rejects.toMatchObject({ error: { code: "DockOntoItself" } });
    await host.send({ type: "DockTab", tabId: "t2", paneId: "p1", side: "up" });
    const tabs = host.workspace().windows[0]!.sideTabs[0]!.tabs;
    expect(tabs.map((t) => t.id)).toEqual(["t1"]);
    expect(panesOf(tabs[0]!.layout)).toEqual(["p2", "p1"]);
  });

  it("turns a pane into a tab, but never a tab's only pane", async () => {
    const host = createMockHost(seed());
    await expect(host.send({ type: "PaneToTab", paneId: "p1", index: 0 })).rejects.toMatchObject({ error: { code: "OnlyPane" } });
    await host.send({ type: "DockTab", tabId: "t2", paneId: "p1", side: "right" });
    await host.send({ type: "PaneToTab", paneId: "p2", index: 0 });
    const tabs = host.workspace().windows[0]!.sideTabs[0]!.tabs;
    expect(tabs.map((t) => panesOf(t.layout))).toEqual([["p2"], ["p1"]]);
  });
});

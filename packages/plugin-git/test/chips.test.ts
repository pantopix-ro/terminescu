import { describe, expect, it } from "vitest";
import type { Decoration } from "@terminescu/host";
import { compose } from "@terminescu/plugin-api";
import { git, markOf, shortenMiddle, type Change } from "../src/index";

const chip = (target: Decoration["target"], key: string, text: string, tip?: string): Decoration => ({
  plugin: "git",
  target,
  slot: "chip",
  key,
  text,
  tone: "neutral",
  ...(tip ? { tip } : {}),
});

const composed = (decorations: Decoration[]) => compose({ plugins: [git], enabled: new Set(["git"]), decorations, appActions: [] });

describe("chips", () => {
  it("a long branch shortens in the middle and keeps its marks", () => {
    expect(shortenMiddle("feature/PROJ-482-retry")).toBe("feature/PROJ-482-retry");
    const long = "feature/PROJ-4821-retry-failed-payments-after-timeout";
    const short = shortenMiddle(long);
    expect(Array.from(short)).toHaveLength(32);
    expect(short.startsWith("feature/PROJ-482")).toBe(true);
    expect(short.endsWith("after-timeout")).toBe(true);
    const c = composed([chip({ kind: "tab", id: "t1" }, "branch", `${long} • ↑2`, "Upstream origin/main")]);
    const [item] = c.tabItems({ id: "t1", sideTabId: "s1", paneIds: [] });
    expect(item!.text).toBe(`${short} • ↑2`);
    expect(item!.tip).toBe(`${long}\nUpstream origin/main`);
  });

  it("a one-tab row shows its tab's chips; a folder its shown tab's", () => {
    const c = composed([chip({ kind: "tab", id: "t1" }, "branch", "main"), chip({ kind: "tab", id: "t2" }, "branch", "develop"), chip({ kind: "tab", id: "t2" }, "pr", "#918")]);
    expect(c.rowItems({ id: "s1", tabIds: ["t1"] }).map((i) => i.text)).toEqual(["main"]);
    expect(c.rowItems({ id: "s2", tabIds: ["t1", "t2"], shownTabId: "t2" }).map((i) => i.text)).toEqual(["develop", "#918"]);
    expect(c.rowItems({ id: "s2", tabIds: ["t1", "t2"] }).map((i) => i.text)).toEqual([]);
    expect(c.tabItems({ id: "t1", sideTabId: "s2", paneIds: [] }).map((i) => i.text)).toEqual(["main"]);
  });
});

describe("marks", () => {
  const change = (c: Partial<Change>): Change => ({ path: "a", staged: false, unstaged: false, untracked: false, conflicted: false, binary: false, ...c });
  it("a conflict wins, then untracked, staged and unstaged", () => {
    expect(markOf(change({ conflicted: true, staged: true })).text).toBe("!");
    expect(markOf(change({ untracked: true })).text).toBe("U");
    expect(markOf(change({ staged: true, unstaged: true })).text).toBe("SM");
    expect(markOf(change({ staged: true })).text).toBe("S");
    expect(markOf(change({ unstaged: true })).text).toBe("M");
  });
});

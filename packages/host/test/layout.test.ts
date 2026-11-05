import { describe, expect, it } from "vitest";
import { isSessionId, panesOf, removePane, resizeSplit, splitPane as trySplit, TOTAL } from "../src/layout";
import type { Layout } from "../src/protocol";

const pane = (id: string): Layout => ({ type: "pane", id });
const splitPane = (...args: Parameters<typeof trySplit>) => trySplit(...args) as Layout | undefined;

/** Same picture as crates/core/tests/layout.rs. */
function shape(l: Layout): string {
  if (l.type === "pane") return l.id;
  const name = l.dir === "row" ? "row" : "col";
  return `${name}(${l.children.map((c, i) => `${shape(c)} ${l.sizes[i]}`).join(", ")})`;
}

function sound(l: Layout): boolean {
  if (l.type === "pane") return true;
  return (
    l.children.length >= 2 &&
    l.sizes.length === l.children.length &&
    l.sizes.reduce((a, b) => a + b, 0) === TOTAL &&
    l.children.every(sound)
  );
}

describe("split tree, like the core", () => {
  it("splits, adds siblings and wraps", () => {
    let l = splitPane(pane("a"), "a", "row", "b", "s1")!;
    expect(shape(l)).toBe("row(a 500, b 500)");
    l = splitPane(l, "a", "row", "c", "s2")!;
    expect(shape(l)).toBe("row(a 250, c 250, b 500)");
    l = splitPane(l, "b", "column", "d", "s3")!;
    expect(shape(l)).toBe("row(a 250, c 250, col(b 500, d 500) 500)");
    expect(splitPane(l, "x", "row", "e", "s4")).toBeUndefined();
  });

  it("removes, collapses and merges same-direction splits", () => {
    let l = splitPane(pane("a"), "a", "row", "b", "s1")!;
    l = splitPane(l, "b", "column", "c", "s2")!;
    l = splitPane(l, "c", "row", "d", "s3")!;
    expect(shape(l)).toBe("row(a 500, col(b 500, row(c 500, d 500) 500) 500)");
    l = removePane(l, "b")!;
    expect(shape(l)).toBe("row(a 500, c 250, d 250)");
    expect(removePane(pane("a"), "a")).toBeNull();
    expect(removePane(pane("a"), "x")).toBeUndefined();
  });

  it("resizes within bounds", () => {
    const l = splitPane(pane("a"), "a", "row", "b", "s1")!;
    expect(shape(resizeSplit(l, "s1", [30, 70]) as Layout)).toBe("row(a 300, b 700)");
    expect(resizeSplit(l, "s1", [1, 999])).toBe("bad");
    expect(resizeSplit(l, "s1", [1])).toBe("bad");
    expect(resizeSplit(l, "nope", [1, 1])).toBe("unknown");
  });

  it("stays sound over many splits and removals", () => {
    let l = pane("p0");
    const live = ["p0"];
    for (let i = 1; i < 40; i++) {
      const target = live[(i * 7) % live.length]!;
      const next = trySplit(l, target, i % 3 === 0 ? "column" : "row", `p${i}`, `s${i}`);
      if (next === "small") continue;
      l = next!;
      live.push(`p${i}`);
      expect(sound(l)).toBe(true);
      if (i % 4 === 0) {
        const [gone] = live.splice((i * 5) % live.length, 1);
        l = removePane(l, gone!)!;
        expect(sound(l)).toBe(true);
      }
    }
    expect(panesOf(l).sort()).toEqual(live.sort());
  });

  it("refuses a split that would leave a share under the minimum", () => {
    let l = pane("a");
    for (let i = 0; i < 4; i++) l = splitPane(l, "a", "row", `n${i}`, `s${i}`)!;
    expect(shape(l)).toBe("row(a 63, n3 62, n2 125, n1 250, n0 500)");
    expect(trySplit(l, "a", "row", "x", "sx")).toBe("small");
    expect(panesOf(splitPane(l, "a", "column", "y", "sy")!)).toContain("y");
  });

  it("rounds a merge like the core (rounding_matches_the_typescript_host)", () => {
    let l = splitPane(pane("a"), "a", "row", "b", "s1")!;
    l = resizeSplit(l, "s1", [333, 667]) as Layout;
    l = splitPane(l, "b", "column", "c", "s2")!;
    l = splitPane(l, "c", "row", "d", "s3")!;
    l = splitPane(l, "d", "row", "e", "s4")!;
    expect(shape(removePane(l, "b")!)).toBe("row(a 333, c 335, d 166, e 166)");
  });

  it("accepts only session ids that cannot read as an option", () => {
    expect(isSessionId("00000000-0000-4000-8000-000000000001")).toBe(true);
    expect(isSessionId("sess-0b5c")).toBe(true);
    expect(isSessionId("--dangerously-skip-permissions")).toBe(false);
    expect(isSessionId("a b")).toBe(false);
    expect(isSessionId("")).toBe(false);
    expect(isSessionId("a".repeat(129))).toBe(false);
  });
});

import { describe, expect, it } from "vitest";
import { dragSizes } from "../src/SplitView";
import { tabChord } from "../src/SideTabList";

describe("dragSizes", () => {
  it("moves the border and keeps the pair's total", () => {
    expect(dragSizes([500, 500], 0, -120)).toEqual([380, 620]);
    expect(dragSizes([300, 300, 400], 1, 50)).toEqual([300, 350, 350]);
  });
  it("stops both sides at the minimum", () => {
    expect(dragSizes([500, 500], 0, -900)).toEqual([50, 950]);
    expect(dragSizes([500, 500], 0, 900)).toEqual([950, 50]);
  });
});

describe("tabChord", () => {
  it("numbers the first eight and gives the last one ⌘9", () => {
    expect([0, 1, 2].map((i) => tabChord(i, 3))).toEqual(["⌘1", "⌘2", "⌘9"]);
    expect(tabChord(0, 1)).toBe("⌘1");
    expect(tabChord(8, 12)).toBeUndefined();
    expect(tabChord(11, 12)).toBe("⌘9");
  });
});

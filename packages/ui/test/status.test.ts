import type { Shown, SideTab, Tint } from "@terminescu/host";
import { describe, expect, it } from "vitest";
import { chipOf, chipStyle, sortSideTabs } from "../src/status";

const side = (id: string, title: string): SideTab => ({ id, title, tabs: [] });
const sides = [side("a", "zeta"), side("b", "alpha"), side("c", "Mid"), side("d", "beta")];
const tagged = (background: Tint | null, auto: Shown["auto"] = "idle"): Shown => ({
  auto,
  waiting: false,
  source: "manual",
  look: { label: "Tag", accent: null, background },
});
const statuses: Record<string, Shown> = {
  a: tagged("red", "done"),
  b: tagged("blue"),
  c: { auto: "running", waiting: false, source: "auto" },
  d: { ...tagged("red", "idle"), waiting: true },
};
const order = (mode: Parameters<typeof sortSideTabs>[2]) => sortSideTabs(sides, statuses, mode, (s) => s.title!).map((s) => s.title);

describe("sidebar sort", () => {
  it("keeps manual order and sorts by name without case", () => {
    expect(order("manual")).toEqual(["zeta", "alpha", "Mid", "beta"]);
    expect(order("name")).toEqual(["alpha", "beta", "Mid", "zeta"]);
  });

  it("groups by tint in tint order, untinted last, by name inside", () => {
    expect(order("background")).toEqual(["alpha", "beta", "zeta", "Mid"]);
  });

  it("puts waiting first, then the most urgent auto status", () => {
    expect(order("status")).toEqual(["beta", "Mid", "zeta", "alpha"]);
  });
});

describe("status chips", () => {
  it("shows the built-in look, renamed when Settings says so", () => {
    expect(chipOf({ auto: "running", waiting: false, source: "auto" })).toMatchObject({ label: "Running", accent: "blue", set: false });
    expect(chipOf({ auto: "done", waiting: false, source: "auto" }, { done: { label: "Shipped", accent: "cyan", background: null } }).label).toBe("Shipped");
    expect(chipOf({ auto: "waiting", waiting: true, source: "auto" }).need).toBe(true);
  });

  it("keeps a set status in its colors while a session waits", () => {
    const chip = chipOf({ ...tagged("magenta"), waiting: true });
    expect(chip).toMatchObject({ label: "Tag", set: true, need: false });
    expect(chipStyle(chip)).toEqual({ "--s-bg": "var(--tint-magenta)" });
    expect(chipStyle({ label: "x", accent: "fg", background: null })).toEqual({ "--s-fg": "var(--fg)", "--s-edge": "var(--fg)" });
  });
});

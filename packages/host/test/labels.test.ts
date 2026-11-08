import { describe, expect, it } from "vitest";
import { homePath, NAMES, paneLabel, programOf, titleOf } from "../src/labels";
import { createMockHost } from "../src/mock";
import type { PaneLabel, Workspace } from "../src/protocol";

// Task 8.30: the mock mirrors the core's labels and names, so the e2e tests hold for both.

describe("pane labels", () => {
  it("writes home as a tilde", () => {
    expect(homePath("/Users/owner/work/api", "/Users/owner")).toBe("~/work/api");
    expect(homePath("/Users/owner", "/Users/owner/")).toBe("~");
    expect(homePath("/Users/owner2/x", "/Users/owner")).toBe("/Users/owner2/x");
  });

  it("keeps a program's name and drops its arguments", () => {
    expect(programOf("npm run dev")).toBe("npm");
    expect(programOf("TOKEN=tok-example curl -H x")).toBe("curl");
    expect(programOf("sudo -E /usr/bin/vim /etc/hosts")).toBe("vim");
    expect(programOf("  ")).toBeUndefined();
  });

  it("strips the spinner from a title and shows it only while Claude runs", () => {
    expect(titleOf("◐ Fix PROJ-482")).toBe("Fix PROJ-482");
    expect(titleOf("✳ ")).toBeUndefined();
    expect(paneLabel("/Users/owner/work/api", "claude", "Fix PROJ-482", true)).toEqual({ path: "~/work/api", program: "claude", title: "Fix PROJ-482" });
    expect(paneLabel("/Users/owner/work/api", undefined, "owner@host: ~", false)).toEqual({ path: "~/work/api" });
    expect(paneLabel(undefined, "claude", "Claude Code", true)).toEqual({ program: "claude" });
  });
});

describe("mock labels and names", () => {
  const seed: Workspace = {
    windows: [{ id: "w1", sideTabs: [{ id: "s1", title: null, name: "Otter", tabs: [{ id: "t1", title: null, layout: { type: "pane", id: "p1" } }] }] }],
  };

  it("sends a pane's path, program and Claude's title as they change", async () => {
    const host = createMockHost(seed);
    const heard: Record<string, PaneLabel>[] = [];
    await host.onLabels((l) => heard.push(l));
    expect(await host.labels()).toEqual({ p1: { path: "~", process: "zsh" } });
    host.setCwd("p1", "/Users/owner/work/api");
    host.runCommand("p1", "npm run dev", null);
    expect(heard.at(-1)).toEqual({ p1: { path: "~/work/api", program: "npm", process: "npm" } });
    host.runCommand("p1", "npm run dev", 0);
    host.runCommand("p1", "claude", null);
    host.setAgent("p1", { session: "s-1", state: "running", lastMessage: null });
    host.setTitle("p1", "◑ Fix PROJ-482");
    expect(heard.at(-1)).toEqual({ p1: { path: "~/work/api", program: "claude", title: "Fix PROJ-482", process: "claude" } });
    const before = heard.length;
    host.setTitle("p1", "◐ Fix PROJ-482");
    expect(heard).toHaveLength(before);
  });

  it("names each new side tab after an animal or bird no other side tab has, and numbers only when they run out", async () => {
    const host = createMockHost(seed);
    for (let i = 0; i < NAMES.length + 2; i++) await host.send({ type: "CreateSideTab", windowId: "w1" });
    const sides = host.workspace().windows[0]!.sideTabs;
    expect(sides.every((s) => s.tabs.every((t) => !("name" in t)))).toBe(true);
    const names = sides.slice(1).map((s) => s.name!);
    expect(new Set([...names, "Otter"]).size).toBe(names.length + 1);
    // "Otter" is taken by the seed, so the last three new rows get a number.
    expect(names.filter((n) => / \d+$/.test(n))).toHaveLength(3);
    expect(names.slice(0, NAMES.length - 1).every((n) => (NAMES as readonly string[]).includes(n))).toBe(true);
  });
});

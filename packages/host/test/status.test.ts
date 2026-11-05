import { describe, expect, it } from "vitest";
import { createMockHost, glob, lookError } from "../src/mock";
import type { Workspace } from "../src/protocol";
import { computeStatuses, paneAuto } from "../src/status";

const ws: Workspace = {
  windows: [
    {
      id: "w1",
      sideTabs: [
        {
          id: "s1",
          title: "PROJ-482",
          status: { label: "Blocked", accent: "red", background: null },
          tabs: [
            { id: "t1", title: null, layout: { type: "pane", id: "p1" } },
            { id: "t2", title: null, layout: { type: "pane", id: "p2" } },
          ],
        },
      ],
    },
  ],
};

describe("statuses", () => {
  it("follows the agent, then the shell", () => {
    expect(paneAuto(undefined)).toBe("idle");
    expect(paneAuto({ shell: { running: false, lastExit: 0 } })).toBe("done");
    expect(paneAuto({ shell: { running: false, lastExit: 2 } })).toBe("failed");
    expect(paneAuto({ agent: "waiting", shell: { running: true, lastExit: null } })).toBe("waiting");
  });

  it("puts manual over dynamic over auto, and amber over all", () => {
    const facts = new Map([
      ["p1", { shell: { running: true, lastExit: null } }],
      ["p2", { agent: "waiting" as const }],
    ]);
    const dynamic = new Map([["p1", { look: { label: "Tests red", accent: "red" as const, background: null }, source: "rule" as const, order: 1 }]]);
    const out = computeStatuses(ws, facts, dynamic);
    expect(out.t1).toMatchObject({ source: "rule", auto: "running", waiting: false });
    expect(out.t2).toMatchObject({ source: "auto", auto: "waiting", waiting: true });
    expect(out.s1).toMatchObject({ source: "manual", auto: "waiting", waiting: true, look: { label: "Blocked" } });
  });

  it("matches commands and checks looks like the core", () => {
    expect(glob("pnpm test*", "pnpm test --filter api")).toBe(true);
    expect(glob("pnpm test*", "npm test")).toBe(false);
    expect(glob("make ?", "make a")).toBe(true);
    expect(glob("a.b", "axb")).toBe(false);
    expect(lookError({ label: "x".repeat(25), accent: null, background: null })).toMatch(/24/);
    expect(lookError({ label: "Busy", accent: "amber" as never, background: null })).toMatch(/accent/);
    expect(lookError({ label: "Waiting on QA", accent: "fg", background: "cyan" })).toBeUndefined();
  });
});

describe("mock statuses", () => {
  it("keeps a rule status under a newer CLI one, and drops it with its disabled rule", async () => {
    const host = createMockHost();
    const side = host.workspace().windows[0]!.sideTabs[0]!;
    const pane = (side.tabs[0]!.layout as { id: string }).id;
    const red = await host.saveStatus(null, { label: "Tests red", accent: "red", background: null });
    const rule = await host.addRule(red.id, { kind: "exitCode", pattern: "pnpm test*" });
    host.runCommand(pane, "pnpm test", 1);
    host.setCliStatus(pane, { label: "Migrating", accent: "cyan", background: null });
    expect((await host.statuses())[side.id]?.look?.label).toBe("Migrating");
    host.setCliStatus(pane, null);
    expect((await host.statuses())[side.id]?.look?.label).toBe("Tests red");
    await host.toggleRule(rule.id, false);
    expect((await host.statuses())[side.id]).toMatchObject({ source: "auto", auto: "failed" });
  });
});

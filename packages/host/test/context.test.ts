import { describe, expect, it, vi } from "vitest";
import { createMockHost } from "../src/mock";
import type { Workspace } from "../src/protocol";

const ws: Workspace = {
  windows: [{ id: "w1", sideTabs: [{ id: "s1", title: "PROJ-482", tabs: [{ id: "t1", title: null, layout: { type: "pane", id: "p1" } }] }] }],
};

describe("context variables (D21)", () => {
  it("keeps one name per side tab and never hands a secret back", async () => {
    const host = createMockHost(ws);
    await host.addVariable("s1", { name: "ticket", value: "PROJ-482", mode: "saved", source: "typed" });
    const secret = await host.addVariable("s1", { name: "API_TOKEN", value: "tok-example", mode: "secret", source: "typed" });
    expect(secret.value).toBeNull();
    await expect(host.addVariable("s1", { name: "ticket", value: "x", mode: "session", source: "typed" })).rejects.toThrow(/already/);
    await expect(host.changeVariable("s1", "API_TOKEN", { mode: "saved" })).rejects.toThrow(/stays/);
    const listed = await host.variables("s1");
    expect(listed.map((v) => [v.name, v.mode, v.value])).toEqual([
      ["ticket", "saved", "PROJ-482"],
      ["API_TOKEN", "secret", null],
    ]);
    expect(JSON.stringify(listed)).not.toContain("tok-example");
    const moved = await host.changeVariable("s1", "ticket", { mode: "secret" });
    expect([moved.mode, moved.value]).toEqual(["secret", null]);
  });

  it("drops a variable once its TTL passes", async () => {
    vi.useFakeTimers();
    try {
      const host = createMockHost(ws);
      await host.addVariable("s1", { name: "from_date", value: "2026-10-01", mode: "session", ttlSecs: 60, source: "typed" });
      expect((await host.variables("s1"))[0]?.expiresAt).toBeTypeOf("number");
      vi.advanceTimersByTime(61_000);
      expect(await host.variables("s1")).toEqual([]);
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps a link once and a terminal pin with its rows", async () => {
    const host = createMockHost(ws);
    const url = "https://github.com/acme/api/pull/918";
    const a = await host.addItem("s1", "link", url, "pasted");
    const b = await host.addItem("s1", "link", ` ${url}`, "auto");
    expect(b.id).toBe(a.id);
    await expect(host.addItem("s1", "link", "not a link", "pasted")).rejects.toThrow(/http/);
    const pin = await host.addPin({
      sideTabId: "s1",
      originTabId: "t1",
      session: null,
      entryId: null,
      paneId: "p1",
      rowStart: 4,
      rowEnd: 5,
      exact: "curl -s localhost:4800/audit/export",
      prefix: null,
      suffix: null,
      start: null,
      end: null,
    });
    expect((await host.pins("s1"))[0]).toEqual(pin);
  });
});

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

describe("context links (refine-context-links)", () => {
  it("cleans a link the way the core does", async () => {
    const { cleanLink } = await import("../src/links");
    expect(cleanLink("https://me:tok@Docs.Example.test:443/guide/?utm_source=mail&page=2#setup")).toBe("https://docs.example.test/guide?page=2#setup");
    expect(cleanLink("https://github.com/Acme/Shop-API/pull/918/files#diff-3a")).toBe("https://github.com/acme/shop-api/pull/918");
    expect(cleanLink("https://claude.ai/code/artifact/abc-123?tab=1")).toBe("https://claude.ai/code/artifact/abc-123");
    expect(cleanLink("https://example.test/s?path=%2Fa%2Fb&x=%7E")).toBe("https://example.test/s?path=%2Fa%2Fb&x=%7E");
    expect(cleanLink("https://files.example.test/a.zip?sig=abc")).toBe("https://files.example.test/a.zip?sig=abc");
    expect(cleanLink("ftp://files.example.test/a")).toBeNull();
    expect(cleanLink("not a link")).toBeNull();
  });

  it("keeps two forms of one PR once, labels it, and shows a name and badge", async () => {
    const host = createMockHost(ws);
    const pr = "https://github.com/acme/shop-api/pull/918";
    await host.addItem("s1", "link", `${pr}/files`, "pasted");
    await host.addItem("s1", "link", `${pr}?utm_source=x`, "pasted");
    let items = await host.items("s1");
    expect(items.map((i) => [i.value, i.label, i.linkType])).toEqual([[pr, "PR #918 · shop-api", "pr"]]);
    host.describeLink("s1", pr, "Retry failed payments", { text: "open", tone: "bad", tip: "PR #918" });
    items = await host.items("s1");
    expect([items[0]!.label, items[0]!.name, items[0]!.badge?.text]).toEqual(["Retry failed payments", "Retry failed payments", "open"]);
  });
});

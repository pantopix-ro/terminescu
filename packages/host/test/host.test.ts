import { describe, expect, it } from "vitest";
import { createMockHost } from "../src/mock";
import type { Host } from "../src/host";
import type { PatchBatch, Workspace } from "../src/protocol";
import { createWorkspaceStore } from "../src/store";

const seed: Workspace = {
  windows: [
    {
      id: "w1",
      sideTabs: [
        { id: "s1", title: "acme-api", tabs: [{ id: "t1", title: null }, { id: "t2", title: null }] },
        { id: "s2", title: null, tabs: [{ id: "t3", title: null }] },
      ],
    },
  ],
};

const tabIds = (ws: Workspace) => ws.windows[0]!.sideTabs.map((s) => s.tabs.map((t) => t.id));

describe("workspace store", () => {
  it("applies a MoveTab patch in two store instances", async () => {
    const host = createMockHost(seed);
    const a = createWorkspaceStore(host);
    const b = createWorkspaceStore(host);
    await Promise.all([a.start(), b.start()]);

    await host.send({ type: "MoveTab", tabId: "t2", toSideTabId: "s2", index: 1 });

    for (const s of [a, b]) {
      expect(s.store.getState().rev).toBe(2);
      expect(tabIds(s.store.getState().workspace)).toEqual([["t1"], ["t3", "t2"]]);
    }
    expect(a.store.getState().workspace).toEqual(host.workspace());
  });

  it("closes the side tab whose last tab moves out", async () => {
    const host = createMockHost(seed);
    const s = createWorkspaceStore(host);
    await s.start();
    await host.send({ type: "MoveTab", tabId: "t3", toSideTabId: "s1", index: 0 });
    expect(tabIds(s.store.getState().workspace)).toEqual([["t3", "t1", "t2"]]);
  });

  it("resyncs from a snapshot after a gap in revs", async () => {
    const host = createMockHost(seed);
    let deliver: ((b: PatchBatch) => void) | undefined;
    // A host that drops the batch for rev 2 on its way to the store.
    const lossy: Host = {
      snapshot: () => host.snapshot(),
      send: (c) => host.send(c),
      onPatches: (l) => {
        deliver = l;
        return host.onPatches((b) => b.rev !== 2 && l(b));
      },
    };
    const s = createWorkspaceStore(lossy);
    await s.start();
    await host.send({ type: "CreateTab", sideTabId: "s2" });
    await host.send({ type: "RenameTab", tabId: "t1", title: "api" });
    await new Promise((r) => setTimeout(r, 0));
    expect(s.store.getState().rev).toBe(3);
    expect(s.store.getState().workspace).toEqual(host.workspace());
    expect(deliver).toBeDefined();
  });

  it("buffers batches that arrive before the snapshot", async () => {
    const host = createMockHost(seed);
    const s = createWorkspaceStore(host);
    const started = s.start();
    await host.send({ type: "CreateTab", sideTabId: "s1" });
    await started;
    expect(s.store.getState().workspace).toEqual(host.workspace());
  });

  it("retries a failed snapshot", async () => {
    const host = createMockHost(seed);
    let failures = 1;
    const flaky: Host = {
      ...host,
      snapshot: () => (failures-- > 0 ? Promise.reject(new Error("busy")) : host.snapshot()),
      onPatches: (l) => host.onPatches(l),
    };
    const s = createWorkspaceStore(flaky);
    await s.start();
    expect(s.store.getState().ready).toBe(false);
    await new Promise((r) => setTimeout(r, 600));
    expect(s.store.getState().ready).toBe(true);
  });

  it("rejects an unknown tab like the core", async () => {
    const host = createMockHost(seed);
    await expect(host.send({ type: "CloseTab", tabId: "nope" })).rejects.toThrow("UnknownTab nope");
  });
});

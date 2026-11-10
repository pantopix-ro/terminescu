import { describe, expect, it } from "vitest";
import { createMockHost, type FakePlugin } from "../src/mock";
import type { Decoration, PluginInfo } from "../src/protocol";

const echo: FakePlugin = {
  id: "echo",
  name: "Echo",
  call(method, args) {
    if (method === "echo") return { got: args };
    throw new Error(`${method} is not a method of this plugin`);
  },
};
const other: FakePlugin = { id: "other", name: "Other" };

const chip = (plugin: string, key: string, text: string): Decoration => ({
  plugin,
  target: { kind: "sideTab", id: "s1" },
  slot: "chip",
  key,
  text,
  tone: "neutral",
});

describe("mock plugins", () => {
  it("answers calls and rejects unknown methods and plugins", async () => {
    const host = createMockHost(undefined, { plugins: [echo, other] });
    expect(await host.pluginCall("echo", "echo", { a: 1 })).toEqual({ got: { a: 1 } });
    await expect(host.pluginCall("echo", "nope")).rejects.toThrow("nope is not a method");
    await expect(host.pluginCall("missing", "echo")).rejects.toThrow("no plugin missing");
    expect(host.pluginCalls).toEqual(["echo.echo", "echo.nope"]);
  });

  it("merges decorations by key and sends the whole list", async () => {
    const host = createMockHost(undefined, { plugins: [echo, other] });
    const seen: Decoration[][] = [];
    await host.onDecorations((d) => seen.push(d));
    host.decorate(chip("echo", "branch", "main"));
    host.decorate(chip("echo", "branch", "feature/x"));
    host.decorate(chip("other", "branch", "other"));
    expect((await host.decorations()).map((d) => `${d.plugin}:${d.text}`)).toEqual(["echo:feature/x", "other:other"]);
    expect(seen).toHaveLength(3);
  });

  it("switching a plugin off drops its decorations, events and calls", async () => {
    const host = createMockHost(undefined, { plugins: [echo, other] });
    const lists: PluginInfo[][] = [];
    await host.onPlugins((p) => lists.push(p));
    const events: string[] = [];
    await host.onPluginEvent("echo", (e, p) => events.push(`${e}:${JSON.stringify(p)}`));
    await host.onPluginEvent("other", (e) => events.push(`other:${e}`));
    host.decorate(chip("echo", "k", "x"));
    host.pluginEmit("echo", "changed", { n: 1 });
    expect(events).toEqual(['changed:{"n":1}']);

    const after = await host.setPluginEnabled("echo", false);
    expect(after.find((p) => p.id === "echo")?.enabled).toBe(false);
    expect(lists).toHaveLength(1);
    expect(await host.decorations()).toEqual([]);
    host.decorate(chip("echo", "k", "x"));
    host.pluginEmit("echo", "changed", {});
    expect(await host.decorations()).toEqual([]);
    expect(events).toHaveLength(1);
    await expect(host.pluginCall("echo", "echo")).rejects.toThrow("off or paused");

    await host.setPluginEnabled("echo", true);
    expect(await host.pluginCall("echo", "echo", 2)).toEqual({ got: 2 });
  });

  it("a paused plugin shows as paused until switched off and on", async () => {
    const host = createMockHost(undefined, { plugins: [echo] });
    host.decorate(chip("echo", "k", "x"));
    host.pausePlugin("echo");
    expect(await host.plugins()).toEqual([{ id: "echo", name: "Echo", description: "", enabled: true, paused: true }]);
    expect(await host.decorations()).toEqual([]);
    await host.setPluginEnabled("echo", false);
    await host.setPluginEnabled("echo", true);
    expect((await host.plugins())[0]?.paused).toBe(false);
  });

  it("a host with no plugins lists none", async () => {
    const host = createMockHost();
    expect(await host.plugins()).toEqual([]);
    expect(await host.decorations()).toEqual([]);
  });
});

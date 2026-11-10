import { describe, expect, it } from "vitest";
import type { PluginBridge } from "@terminescu/plugin-api";
import { githubFlags } from "../src/github";

/** A bridge whose `status` answers when the test resolves it, and whose events the test sends. */
function bridge() {
  const listeners = new Set<(p: unknown) => void>();
  let answer!: (s: { github: boolean }) => void;
  const b: PluginBridge = {
    call: <T>() => new Promise<T>((r) => (answer = r as never)),
    on(_, l) {
      listeners.add(l);
      return () => listeners.delete(l);
    },
  };
  return { b, answer: (github: boolean) => answer({ github }), send: (on: boolean) => listeners.forEach((l) => l({ on })), listeners };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

describe("the github flag", () => {
  it("follows status, then each event", async () => {
    const t = bridge();
    const seen: string[][] = [];
    githubFlags(t.b, (f) => seen.push([...f]));
    t.answer(true);
    await tick();
    t.send(false);
    expect(seen).toEqual([["github"], []]);
  });

  it("keeps an event over a status that answers after it", async () => {
    const t = bridge();
    const seen: string[][] = [];
    githubFlags(t.b, (f) => seen.push([...f]));
    t.send(false);
    t.answer(true);
    await tick();
    expect(seen).toEqual([[]]);
  });

  it("stops listening when stopped", async () => {
    const t = bridge();
    const seen: string[][] = [];
    const stop = githubFlags(t.b, (f) => seen.push([...f]));
    stop();
    t.answer(true);
    await tick();
    expect(t.listeners.size).toBe(0);
    expect(seen).toEqual([]);
  });
});

import { describe, expect, it } from "vitest";
import type { Answer, PluginBridge, Question } from "@terminescu/plugin-api";
import { factsFor, parseSource } from "../src/DiffView";
import { beforeRowClose, offerOrphan, type Orphan } from "../src/worktree";

/** A Rust half with one marked row whose worktree has `changed` files, and scripted answers. */
function setup(changed: number | null, answers: (string | null)[], marked = true) {
  const calls: [string, unknown][] = [];
  const asked: Question[] = [];
  const bridge: PluginBridge = {
    async call<T>(method: string, args?: unknown) {
      calls.push([method, args]);
      if (method === "rowWorktree") return (marked ? { worktree: "/code/shop-api-x", changed } : null) as T;
      if (method === "worktreeRemove") {
        const force = (args as { force?: boolean }).force === true;
        return (changed && !force ? { removed: false, changed } : { removed: true }) as T;
      }
      throw new Error(method);
    },
    on: () => () => undefined,
  };
  const ask = async (q: Question): Promise<Answer> => {
    asked.push(q);
    return { choice: answers.shift() ?? null, values: {} };
  };
  const run = () => beforeRowClose({ sideTabId: "s2", title: "PROJ-490", bridge, ask });
  return { run, calls, asked };
}

const removes = (calls: [string, unknown][]) => calls.filter(([m]) => m === "worktreeRemove").map(([, a]) => a);

describe("closing a worktree row", () => {
  it("asks nothing for an unmarked row", async () => {
    const t = setup(0, [], false);
    expect(await t.run()).toBe(true);
    expect(t.asked).toEqual([]);
  });

  it("keeps the worktree by default", async () => {
    const t = setup(0, ["keep"]);
    expect(await t.run()).toBe(true);
    expect(t.asked[0]?.choices[0]?.id).toBe("keep");
    expect(removes(t.calls)).toEqual([]);
  });

  it("keeps the row open on Escape", async () => {
    const t = setup(0, [null]);
    expect(await t.run()).toBe(false);
    expect(removes(t.calls)).toEqual([]);
  });

  it("removes a clean worktree without a second question", async () => {
    const t = setup(0, ["remove"]);
    expect(await t.run()).toBe(true);
    expect(t.asked).toHaveLength(1);
    expect(removes(t.calls)).toEqual([{ row: "s2" }]);
  });

  it("names the changed files and forces only on Remove anyway", async () => {
    const cancel = setup(2, ["remove", null]);
    expect(await cancel.run()).toBe(false);
    expect(cancel.asked[1]?.text).toBe("2 changed files in the worktree would be lost, and so would ignored files such as .env.");
    expect(cancel.asked[1]?.choices[0]?.id).toBe("keep");
    expect(removes(cancel.calls)).toEqual([{ row: "s2" }]);
    const keep = setup(1, ["remove", "keep"]);
    expect(await keep.run()).toBe(true);
    expect(keep.asked[1]?.text).toBe("1 changed file in the worktree would be lost, and so would ignored files such as .env.");
    expect(removes(keep.calls)).toEqual([{ row: "s2" }]);
    const force = setup(2, ["remove", "remove"]);
    expect(await force.run()).toBe(true);
    expect(removes(force.calls)).toEqual([{ row: "s2" }, { row: "s2", force: true }]);
  });
});

describe("a row that cannot answer (polish-git-plugin D5)", () => {
  it("closes when the Rust half fails", async () => {
    const bridge: PluginBridge = { call: () => Promise.reject(new Error("busy")), on: () => () => undefined };
    const ask = async (): Promise<Answer> => ({ choice: null, values: {} });
    expect(await beforeRowClose({ sideTabId: "s9", title: "x", bridge, ask })).toBe(true);
  });

  it("asks to close anyway when the repository cannot be read", async () => {
    const asked: Question[] = [];
    const bridge: PluginBridge = {
      async call<T>(method: string) {
        if (method === "rowWorktree") return { worktree: "/code/shop-api-x", changed: null } as T;
        throw new Error("not a git repository");
      },
      on: () => () => undefined,
    };
    const answers = ["remove", "close"];
    const ask = async (q: Question): Promise<Answer> => (asked.push(q), { choice: answers.shift() ?? null, values: {} });
    expect(await beforeRowClose({ sideTabId: "s8", title: "x", bridge, ask })).toBe(true);
    expect(asked[1]?.text).toBe("not a git repository. Close the row anyway? The worktree stays on disk.");
  });
});

describe("a worktree left behind (polish-git-plugin D6)", () => {
  const orphan: Orphan = { row: "s7", window: "w1", worktree: "/code/shop-api-y", repo: "/code/shop-api", branch: "y" };
  const run = async (o: Orphan, answers: (string | null)[], windowId = "w1", changed = 0) => {
    const calls: unknown[] = [];
    const asked: Question[] = [];
    const bridge: PluginBridge = {
      async call<T>(method: string, args?: unknown) {
        calls.push(args);
        const force = (args as { force?: boolean }).force === true;
        return (changed && !force ? { removed: false, changed } : { removed: true }) as T;
      },
      on: () => () => undefined,
    };
    const ask = async (q: Question): Promise<Answer> => (asked.push(q), { choice: answers.shift() ?? null, values: {} });
    await offerOrphan({ windowId, bridge, ask }, o);
    return { calls, asked };
  };

  it("asks in its own window only, and removes on Remove", async () => {
    expect((await run(orphan, ["remove"], "w2")).asked).toEqual([]);
    const t = await run(orphan, ["remove"]);
    expect(t.asked[0]?.text).toBe("Worktree kept at /code/shop-api-y. Remove it?");
    expect(t.calls).toEqual([{ worktree: "/code/shop-api-y", repo: "/code/shop-api", branch: "y" }]);
  });

  it("keeps by default, and forces only after a second Remove", async () => {
    expect((await run(orphan, [null])).calls).toEqual([]);
    const t = await run(orphan, ["remove", "remove"], "w1", 3);
    expect(t.asked[1]?.text).toBe("3 changed files in the worktree would be lost.");
    expect(t.calls).toHaveLength(2);
    expect((t.calls[1] as { force?: boolean }).force).toBe(true);
  });

  it("raises no notice for a row this window asked about", async () => {
    const bridge: PluginBridge = {
      async call<T>(method: string) {
        return (method === "rowWorktree" ? { worktree: "/code/shop-api-z", changed: 0 } : { removed: true }) as T;
      },
      on: () => () => undefined,
    };
    const keep = async (): Promise<Answer> => ({ choice: "keep", values: {} });
    expect(await beforeRowClose({ sideTabId: "s6", title: "z", bridge, ask: keep })).toBe(true);
    expect((await run({ ...orphan, row: "s6" }, ["remove"])).asked).toEqual([]);
  });
});

describe("diff tab source (polish-git-plugin D3, D7)", () => {
  it("reads only for its own repository", () => {
    expect(factsFor("/code/shop-api", { pane: "p1", root: "/code/shop-api/" })).toBe(true);
    expect(factsFor("/code/shop-api", { pane: "p1", root: "/code/other" })).toBe(false);
    expect(factsFor("/code/shop-api", { pane: "p1", root: null })).toBe(true);
  });

  it("keeps the old path and the deleted flag", () => {
    expect(parseSource(JSON.stringify({ root: "/r", path: "b.ts", from: "a.ts", deleted: true }))).toEqual({ root: "/r", path: "b.ts", from: "a.ts", deleted: true });
    expect(parseSource(JSON.stringify({ root: "/r", path: "b.ts", from: "", deleted: "yes" }))).toEqual({ root: "/r", path: "b.ts" });
  });
});

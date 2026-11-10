import { describe, expect, it } from "vitest";
import type { Answer, PluginBridge, Question } from "@terminescu/plugin-api";
import { beforeRowClose } from "../src/worktree";

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
    expect(cancel.asked[1]?.text).toBe("2 changed files in the worktree would be lost.");
    expect(cancel.asked[1]?.choices[0]?.id).toBe("keep");
    expect(removes(cancel.calls)).toEqual([{ row: "s2" }]);
    const keep = setup(1, ["remove", "keep"]);
    expect(await keep.run()).toBe(true);
    expect(keep.asked[1]?.text).toBe("1 changed file in the worktree would be lost.");
    expect(removes(keep.calls)).toEqual([{ row: "s2" }]);
    const force = setup(2, ["remove", "remove"]);
    expect(await force.run()).toBe(true);
    expect(removes(force.calls)).toEqual([{ row: "s2" }, { row: "s2", force: true }]);
  });
});

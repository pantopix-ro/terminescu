import { describe, expect, it } from "vitest";
import type { Tab } from "@terminescu/host";
import { paneActivity, tabActivities, tabActivity } from "../src/activity";

const pane = (id: string) => ({ type: "pane" as const, id });
const tab = (id: string, ...panes: string[]): Tab => ({
  id,
  title: null,
  layout: panes.length === 1 ? pane(panes[0]!) : { type: "split", id: `${id}-s`, dir: "row", children: panes.map(pane), sizes: [500, 500] },
});
const agent = (state: "idle" | "running" | "waiting" | "done") => ({ session: "s", state, lastMessage: null });

describe("activity", () => {
  it("a running program is busy, an idle shell is not", () => {
    expect(paneActivity("p", { p: { program: "mvn" } }, {})).toBe("busy");
    expect(paneActivity("p", { p: { path: "~" } }, {})).toBeUndefined();
  });

  it("Claude follows its hook state, not the claude program", () => {
    const labels = { p: { program: "claude" } };
    expect(paneActivity("p", labels, { p: agent("running") })).toBe("busy");
    expect(paneActivity("p", labels, { p: agent("waiting") })).toBe("wait");
    expect(paneActivity("p", labels, { p: agent("idle") })).toBeUndefined();
    expect(paneActivity("p", labels, { p: agent("done") })).toBeUndefined();
    expect(paneActivity("p", labels, {})).toBeUndefined();
  });

  it("a tab takes the most urgent pane", () => {
    const a = tab("a", "p1", "p2");
    const b = tab("b", "p3");
    const labels = { p1: { program: "npm" }, p3: { program: "claude" } };
    expect(tabActivity(a, labels, { p2: agent("waiting") })).toBe("wait");
    const acts = tabActivities([a, b], labels, { p3: agent("idle") });
    expect(acts).toEqual({ a: "busy" });
  });
});

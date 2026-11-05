import type { AgentView, Tab } from "@terminescu/host";
import { describe, expect, it } from "vitest";
import { tabAgent } from "../src/agents";

const agent = (state: AgentView["state"]): AgentView => ({ session: `sess-${state}`, state, lastMessage: null });

const tab: Tab = {
  id: "t1",
  title: null,
  layout: {
    type: "split",
    id: "s1",
    dir: "row",
    children: [
      { type: "pane", id: "p1" },
      { type: "pane", id: "p2" },
    ],
    sizes: [500, 500],
  },
};

describe("tabAgent", () => {
  it("shows the shell when no pane runs Claude", () => {
    expect(tabAgent(tab, { other: agent("waiting") })).toBeUndefined();
  });
  it("picks the session that needs the person most", () => {
    expect(tabAgent(tab, { p1: agent("done"), p2: agent("waiting") })?.state).toBe("waiting");
    expect(tabAgent(tab, { p1: agent("running"), p2: agent("idle") })?.state).toBe("running");
  });
});

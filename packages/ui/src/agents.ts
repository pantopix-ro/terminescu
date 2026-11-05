import { panesOf, type AgentState, type AgentView, type Tab } from "@terminescu/host";

/** Claude sessions by pane id. */
export type Agents = Readonly<Record<string, AgentView>>;

const URGENCY: Record<AgentState, number> = { waiting: 3, running: 2, done: 1, idle: 0 };

/** The session a tab shows: of its panes' sessions, the one that needs the person most. */
export function tabAgent(tab: Tab, agents: Agents): AgentView | undefined {
  let best: AgentView | undefined;
  for (const p of panesOf(tab.layout)) {
    const a = agents[p];
    if (a && (!best || URGENCY[a.state] > URGENCY[best.state])) best = a;
  }
  return best;
}

/** The dot class for a state (design system `ao-dot`). */
export const DOT: Record<AgentState, string> = { waiting: "need", running: "run", done: "done", idle: "idle" };

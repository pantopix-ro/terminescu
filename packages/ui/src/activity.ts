// What a tab's icon shows while something runs in it (task 8.54): it blinks while a command or
// Claude works, and blinks and bounces while Claude waits for the person.
import { panesOf, type Tab } from "@terminescu/host";
import type { Agents } from "./agents";
import type { Labels } from "./subtitles";

export type Activity = "busy" | "wait";
export type Activities = Readonly<Record<string, Activity>>;

/**
 * One pane: Claude's hook state when the hook reports it, else the shell's running program.
 * Claude blinks only while it works or waits: at its prompt, or with no hook state yet (a
 * session older than the app, the hook off), `claude` running is not busy.
 */
export function paneActivity(pane: string, labels: Labels, agents: Agents): Activity | undefined {
  const agent = agents[pane];
  if (agent) return agent.state === "waiting" ? "wait" : agent.state === "running" ? "busy" : undefined;
  const program = labels[pane]?.program;
  return program && program !== "claude" ? "busy" : undefined;
}

/** A tab takes its most urgent pane: waiting over busy. */
export function tabActivity(tab: Tab, labels: Labels, agents: Agents): Activity | undefined {
  let out: Activity | undefined;
  for (const p of panesOf(tab.layout)) {
    const a = paneActivity(p, labels, agents);
    if (a === "wait") return a;
    out ??= a;
  }
  return out;
}

/** Every tab's activity by tab id; idle tabs are left out. */
export function tabActivities(tabs: Iterable<Tab>, labels: Labels, agents: Agents): Record<string, Activity> {
  const out: Record<string, Activity> = {};
  for (const t of tabs) {
    const a = tabActivity(t, labels, agents);
    if (a) out[t.id] = a;
  }
  return out;
}

/** The icon class for an activity: `act-busy` blinks, `act-wait` blinks and bounces. */
export const actClass = (a: Activity | undefined) => (a ? ` act-${a}` : "");

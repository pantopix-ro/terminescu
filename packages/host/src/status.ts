// Statuses as crates/core/src/status.rs computes them (D15). The core sends the result; the mock
// host and tests use this copy.

import { terminalsOf } from "./layout";
import { AUTOS, type AgentState, type Auto, type Id, type Look, type Shown, type StatusSource, type Workspace } from "./protocol";

/** What a terminal pane shows on its own. */
export interface PaneFacts {
  agent?: AgentState;
  /** The shell's last command: running, or how it exited. */
  shell?: { running: boolean; lastExit: number | null };
}

/** A status a rule or `terminescu status` set on a pane; the newest (highest `order`) wins. */
export interface PaneDynamic {
  look: Look;
  source: Extract<StatusSource, "rule" | "cli">;
  order: number;
}

const rank = (a: Auto) => AUTOS.indexOf(a);
const most = (a: Auto, b: Auto) => (rank(b) > rank(a) ? b : a);

export function paneAuto(f: PaneFacts | undefined): Auto {
  if (f?.agent) return f.agent;
  if (!f?.shell) return "idle";
  if (f.shell.running) return "running";
  if (f.shell.lastExit === null) return "idle";
  return f.shell.lastExit === 0 ? "done" : "failed";
}

function resolve(manual: Look | undefined, dynamic: PaneDynamic | undefined, auto: Auto, waiting: boolean): Shown {
  if (manual) return { auto, waiting, source: "manual", look: { label: manual.label, accent: manual.accent, background: manual.background } };
  if (dynamic) return { auto, waiting, source: dynamic.source, look: dynamic.look };
  return { auto, waiting, source: "auto" };
}

function newest(panes: Id[], dynamic: ReadonlyMap<Id, PaneDynamic>): PaneDynamic | undefined {
  let best: PaneDynamic | undefined;
  for (const p of panes) {
    const d = dynamic.get(p);
    if (d && (!best || d.order > best.order)) best = d;
  }
  return best;
}

/** Every tab's and side tab's status, by id. */
export function computeStatuses(
  ws: Workspace,
  facts: ReadonlyMap<Id, PaneFacts>,
  dynamic: ReadonlyMap<Id, PaneDynamic> = new Map(),
): Record<Id, Shown> {
  const out: Record<Id, Shown> = {};
  for (const side of ws.windows.flatMap((w) => w.sideTabs)) {
    let sideAuto: Auto = "idle";
    let sideWaiting = false;
    const sidePanes: Id[] = [];
    for (const tab of side.tabs) {
      const panes = terminalsOf(tab.layout);
      const auto = panes.map((p) => paneAuto(facts.get(p))).reduce(most, "idle");
      const waiting = panes.some((p) => facts.get(p)?.agent === "waiting");
      out[tab.id] = resolve(tab.status, newest(panes, dynamic), auto, waiting);
      sideAuto = most(sideAuto, auto);
      sideWaiting ||= waiting;
      sidePanes.push(...panes);
    }
    out[side.id] = resolve(side.status, newest(sidePanes, dynamic), sideAuto, sideWaiting);
  }
  return out;
}

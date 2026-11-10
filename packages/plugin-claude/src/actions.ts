// The Claude plugin's actions (add-claude-insights 3.1, 3.3): resume a past session of the row,
// and open Claude's memory files. The Rust half knows the paths and the resume line.

import type { ActionContext, PluginAction } from "@terminescu/plugin-api";

/** One past session of a row, as the Rust half's `history` call answers it. */
export interface PastSession {
  id: string;
  cwd: string;
  started: number;
  ended?: number | null;
  prompt?: string | null;
}

const needRow = (cx: ActionContext) => {
  if (!cx.sideTabId) throw new Error("no row is selected");
  return cx.sideTabId;
};

/** When a session started, as "10 Nov 09:14". */
export const startedAt = (secs: number) =>
  new Date(secs * 1000).toLocaleString("en-GB", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

export const resume: PluginAction = {
  id: "resume",
  label: "Resume…",
  menu: "Shell",
  accel: "Cmd+Alt+R",
  rowMenu: true,
  async run(cx) {
    const row = needRow(cx);
    const list = await cx.bridge.call<PastSession[]>("history", { row });
    if (!list.length) {
      await cx.ask({ title: "Resume", text: "No Claude session ran in this row yet.", choices: [{ id: "ok", label: "OK" }] });
      return;
    }
    const a = await cx.ask({
      title: "Resume a Claude session",
      text: "Opens a new tab in the session's folder with the resume command typed. Press Enter there to run it.",
      pick: {
        label: "Session",
        items: list.map((s) => ({ id: s.id, label: s.prompt || "(no prompt)", detail: `${startedAt(s.started)} · ${s.cwd}` })),
      },
      choices: [{ id: "resume", label: "Resume" }],
    });
    if (a.choice !== "resume") return;
    await cx.bridge.call("resume", { row, session: a.values.pick ?? list[0]!.id });
  },
};

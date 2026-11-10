// Worktree rows (add-git-plugin 3.3, D5): make one from a new branch, open every worktree of the
// repository as a row, and ask before a marked row's worktree goes. The Rust half does the git
// work and keeps the marks; this half only asks.

import type { ActionContext, PluginAction, RowCloseContext } from "@terminescu/plugin-api";

const needShell = (cx: ActionContext) => {
  if (!cx.shellPaneId) throw new Error("the row has no shell to read the repository from");
  return cx.shellPaneId;
};

export const newWorktreeRow: PluginAction = {
  id: "newWorktreeRow",
  label: "New Row from Worktree…",
  menu: "Shell",
  accel: "Cmd+Alt+N",
  newMenu: true,
  async run(cx) {
    const pane = needShell(cx);
    const a = await cx.ask({
      title: "New row from worktree",
      text: "Makes the branch and a worktree for it, and opens a row in it below this one.",
      fields: [
        { id: "branch", label: "Branch", placeholder: "PROJ-490-refunds" },
        { id: "base", label: "Base", placeholder: "the current branch" },
      ],
      detail: "git worktree add -b <branch> <folder> <base>",
      choices: [{ id: "create", label: "Create" }],
    });
    if (a.choice !== "create") return;
    const branch = (a.values.branch ?? "").trim();
    if (!branch) throw new Error("a branch name is needed");
    const base = (a.values.base ?? "").trim();
    await cx.bridge.call("worktreeAdd", { pane, branch, base: base || null });
  },
};

export const openWorktreeRows: PluginAction = {
  id: "openWorktrees",
  label: "Open Worktrees as Rows",
  menu: "Shell",
  accel: "Cmd+Alt+Shift+N",
  newMenu: true,
  async run(cx) {
    const pane = needShell(cx);
    const got = await cx.bridge.call<{ opened: unknown[] }>("openWorktrees", { pane });
    if (got.opened.length === 0)
      await cx.ask({ title: "Open worktrees as rows", text: "Every worktree of this repository already has a row.", choices: [{ id: "ok", label: "OK" }] });
  },
};

interface Mark {
  worktree: string;
  repo?: string;
  /** Null when git could not read the worktree. */
  changed: number | null;
}

interface Removed {
  removed: boolean;
  changed?: number | null;
}

const files = (n: number) => (n === 1 ? "1 changed file" : `${n} changed files`);

/**
 * A marked row asks Keep worktree (the default) or Remove worktree. A worktree with changes asks
 * again, naming the count; Keep stays the default there too. No branch is ever deleted. Cancel
 * (Escape) keeps the row open.
 */
export async function beforeRowClose(cx: RowCloseContext): Promise<boolean> {
  const mark = await cx.bridge.call<Mark | null>("rowWorktree", { row: cx.sideTabId });
  if (!mark) return true;
  const first = await cx.ask({
    title: "Close worktree row",
    text: `${cx.title} works in a worktree. Keep it on disk, or remove it? The branch stays either way.`,
    detail: mark.worktree,
    choices: [
      { id: "keep", label: "Keep worktree" },
      { id: "remove", label: "Remove worktree", danger: true },
    ],
  });
  if (first.choice === "keep") return true;
  if (first.choice !== "remove") return false;
  const tried = await cx.bridge.call<Removed>("worktreeRemove", { row: cx.sideTabId });
  if (tried.removed) return true;
  const n = tried.changed ?? mark.changed;
  const second = await cx.ask({
    title: "Remove worktree with changes",
    text: n == null ? "The worktree has changes that would be lost." : `${files(n)} in the worktree would be lost.`,
    detail: mark.worktree,
    choices: [
      { id: "keep", label: "Keep worktree" },
      { id: "remove", label: "Remove anyway", danger: true },
    ],
  });
  if (second.choice === "keep") return true;
  if (second.choice !== "remove") return false;
  const forced = await cx.bridge.call<Removed>("worktreeRemove", { row: cx.sideTabId, force: true });
  return forced.removed;
}

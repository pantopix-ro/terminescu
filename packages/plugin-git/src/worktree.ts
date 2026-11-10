// Worktree rows (add-git-plugin 3.3, D5): make one from a new branch, open every worktree of the
// repository as a row, and ask before a marked row's worktree goes. The Rust half does the git
// work and keeps the marks; this half only asks. A worktree row that closed without asking, as
// when its shell exited, brings a notice that offers to remove the worktree (polish-git-plugin D6).

import type { ActionContext, PluginAction, RowCloseContext, WatchContext } from "@terminescu/plugin-api";

/** Where the Rust half puts a new worktree, `<branch>` standing for the branch. */
interface Where {
  at: string;
  base: string;
}

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

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
    const where = await cx.bridge.call<Where>("worktreeWhere", { pane }).catch(() => undefined);
    const a = await cx.ask({
      title: "New row from worktree",
      text: "Makes the branch and a worktree for it, and opens a row in it below this one.",
      fields: [
        { id: "branch", label: "Branch", placeholder: "PROJ-490-refunds" },
        { id: "base", label: "Base", placeholder: where?.base ?? "the current branch" },
      ],
      detail: where ? `Goes to ${where.at}, from ${where.base} unless Base names another.` : "git worktree add -b <branch> <folder> <base>",
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

/** Rows this window asked about as they closed; their close event raises no notice. */
const asked = new Set<string>();

/**
 * A marked row asks Keep worktree (the default) or Remove worktree. A worktree with changes or ignored files asks
 * again, naming the count; Keep stays the default there too. No branch is ever deleted. Cancel
 * (Escape) keeps the row open.
 */
export async function beforeRowClose(cx: RowCloseContext): Promise<boolean> {
  // A plugin that cannot answer never keeps a row open.
  const mark = await cx.bridge.call<Mark | null>("rowWorktree", { row: cx.sideTabId }).catch(() => null);
  if (!mark) return true;
  const closes = await askAboutWorktree(cx, mark);
  if (closes) asked.add(cx.sideTabId);
  return closes;
}

async function askAboutWorktree(cx: RowCloseContext, mark: Mark): Promise<boolean> {
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
  let tried: Removed;
  try {
    tried = await cx.bridge.call<Removed>("worktreeRemove", { row: cx.sideTabId });
  } catch (e) {
    // An unreadable repository does not block the close; the person decides.
    const stuck = await cx.ask({
      title: "Worktree not removed",
      text: `${message(e)}. Close the row anyway? The worktree stays on disk.`,
      detail: mark.worktree,
      choices: [{ id: "close", label: "Close row" }],
    });
    return stuck.choice === "close";
  }
  if (tried.removed) return true;
  const n = tried.changed ?? mark.changed;
  const second = await cx.ask({
    title: "Remove worktree with changes",
    text:
      n == null
        ? "The worktree has changes or ignored files that would be lost."
        : n === 0
          ? "The worktree holds ignored files, such as .env or build output, that would be lost."
          : `${files(n)} in the worktree would be lost, and so would ignored files such as .env.`,
    detail: mark.worktree,
    choices: [
      { id: "keep", label: "Keep worktree" },
      { id: "remove", label: "Remove anyway", danger: true },
    ],
  });
  if (second.choice === "keep") return true;
  if (second.choice !== "remove") return false;
  const forced = await cx.bridge.call<Removed>("worktreeRemove", { row: cx.sideTabId, force: true });
  if (!forced.removed) throw new Error(`git could not remove the worktree at ${mark.worktree}`);
  return true;
}

/** A worktree row that closed without asking, as the Rust half reports it. */
export interface Orphan {
  row: string;
  /** The window that held the row; null when the plugin knew none of its panes. */
  window: string | null;
  worktree: string;
  repo?: string | null;
  branch?: string | null;
}

/** The notice for a worktree left behind, in the window that held its row. */
export async function offerOrphan(cx: WatchContext, o: Orphan): Promise<void> {
  if (asked.delete(o.row)) return;
  const here = o.window ? o.window === cx.windowId : typeof document !== "undefined" && document.hasFocus();
  if (!here || !o.repo) return;
  const first = await cx.ask({
    title: "Worktree kept",
    text: `Worktree kept at ${o.worktree}. Remove it?`,
    detail: o.branch ? `The branch ${o.branch} stays either way.` : "The branch stays either way.",
    choices: [
      { id: "keep", label: "Keep worktree" },
      { id: "remove", label: "Remove worktree", danger: true },
    ],
  });
  if (first.choice !== "remove") return;
  const args = { worktree: o.worktree, repo: o.repo, branch: o.branch ?? null };
  try {
    const tried = await cx.bridge.call<Removed>("worktreeRemoveAt", args);
    if (tried.removed) return;
    const second = await cx.ask({
      title: "Remove worktree with changes",
      text: tried.changed == null ? "The worktree has changes that would be lost." : `${files(tried.changed)} in the worktree would be lost.`,
      detail: o.worktree,
      choices: [
        { id: "keep", label: "Keep worktree" },
        { id: "remove", label: "Remove anyway", danger: true },
      ],
    });
    if (second.choice !== "remove") return;
    const forced = await cx.bridge.call<Removed>("worktreeRemoveAt", { ...args, force: true });
    if (!forced.removed) throw new Error(`git could not remove the worktree at ${o.worktree}`);
  } catch (e) {
    await cx.ask({ title: "Worktree not removed", text: message(e), choices: [{ id: "ok", label: "OK" }] });
  }
}

/** Follows the Rust half's orphan events while the plugin is on. */
export const watchOrphans = (cx: WatchContext) =>
  cx.bridge.on("orphan", (p) => {
    void offerOrphan(cx, p as Orphan).catch((e: unknown) => console.error("git: orphan worktree", e));
  });

// The GitHub items (add-git-plugin 3.4, D4). They show only while `gh` is installed, logged in
// and allowed in Settings: the Rust half says so in `status` and in each `github` event. `gh`
// keeps its own token; this half never sees one.

import type { ActionContext, PluginAction, PluginBridge } from "@terminescu/plugin-api";

/** The flag the GitHub actions wait for. */
export const GITHUB = "github";

/** An open pull request, as the Rust half answers it. */
export interface PullRequest {
  number: number;
  title: string;
  url: string;
  branch: string;
  checks: string;
}

/** Raises `github` while the Rust half's GitHub part is on. An event wins over a late status. */
export function githubFlags(bridge: PluginBridge, set: (flags: readonly string[]) => void): () => void {
  let gone = false;
  let heard = false;
  const apply = (on: boolean) => !gone && set(on ? [GITHUB] : []);
  const off = bridge.on("github", (p) => {
    heard = true;
    apply((p as { on?: boolean } | null)?.on === true);
  });
  bridge.call<{ github?: boolean }>("status").then(
    (s) => !heard && apply(s.github === true),
    () => !heard && apply(false),
  );
  return () => {
    gone = true;
    off();
  };
}

const needShell = (cx: ActionContext) => {
  if (!cx.shellPaneId) throw new Error("the row has no shell to read the repository from");
  return cx.shellPaneId;
};

const tell = (cx: ActionContext, title: string, text: string) => cx.ask({ title, text, choices: [{ id: "ok", label: "OK" }] }).then(() => undefined);

export const openPullRequest: PluginAction = {
  id: "openPullRequest",
  label: "Open Pull Request",
  menu: "Shell",
  accel: "Cmd+Alt+G",
  when: GITHUB,
  async run(cx) {
    const pane = needShell(cx);
    const got = await cx.bridge.call<{ facts?: { head?: unknown }; pr?: PullRequest | null }>("facts", { pane });
    if (!got.pr) return tell(cx, "Open pull request", "This branch has no open pull request.");
    await cx.openUrl(got.pr.url);
  },
};

export const checkOutPullRequest: PluginAction = {
  id: "checkOutPullRequest",
  label: "Check Out Pull Request as a Row…",
  menu: "Shell",
  accel: "Cmd+Alt+Shift+G",
  newMenu: true,
  when: GITHUB,
  async run(cx) {
    const pane = needShell(cx);
    const prs = await cx.bridge.call<PullRequest[]>("pullRequests", { pane });
    if (prs.length === 0) return tell(cx, "Check out pull request", "The repository has no open pull requests.");
    const sorted = [...prs].sort((a, b) => b.number - a.number);
    const a = await cx.ask({
      title: "Check out pull request",
      text: "Opens the pull request in a new worktree, as a row below this one.",
      pick: { label: "Open pull requests", items: sorted.map((p) => ({ id: String(p.number), label: `#${p.number} ${p.title}`, detail: p.branch })) },
      detail: "gh pr checkout <number>",
      choices: [{ id: "checkout", label: "Check out" }],
    });
    if (a.choice !== "checkout") return;
    const pr = sorted.find((p) => String(p.number) === a.values.pick);
    if (!pr) return;
    await cx.bridge.call("prCheckout", { pane, number: pr.number, branch: pr.branch });
  },
};

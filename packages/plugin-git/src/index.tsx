// The git plugin's UI half (add-git-plugin group 3): branch and pull request chips on tabs and
// rows, the Changes section, the diff tab, worktree rows, the GitHub items and Settings › Git.
// Everything it shows comes from its Rust half.

import { definePlugin } from "@terminescu/plugin-api";
import { ChangesSection } from "./ChangesSection";
import { rowChips, tabChips } from "./chips";
import { DIFF_KIND, DiffView } from "./DiffView";
import { checkOutPullRequest, githubFlags, openPullRequest } from "./github";
import { SettingsPage } from "./SettingsPage";
import { beforeRowClose, newWorktreeRow, openWorktreeRows } from "./worktree";
import "./git.css";

export { ChangesSection, markOf, type Change, type ChangesAnswer } from "./ChangesSection";
export { MAX_BRANCH, rowChips, shortBranch, shortenMiddle, tabChips } from "./chips";
export { DIFF_KIND, DiffView, diffView, parseSource, type BaseText, type DiffSource } from "./DiffView";
export { checkOutPullRequest, GITHUB, githubFlags, openPullRequest, type PullRequest } from "./github";
export { SettingsPage, type GitSettings } from "./SettingsPage";
export { beforeRowClose, newWorktreeRow, openWorktreeRows } from "./worktree";

export const git = definePlugin({
  id: "git",
  name: "Git",
  tabDecorators: [tabChips],
  rowDecorators: [rowChips],
  panelSections: [{ id: "changes", title: "Changes", render: ChangesSection }],
  paneViews: [{ kind: DIFF_KIND, render: DiffView }],
  actions: [
    newWorktreeRow,
    openWorktreeRows,
    openPullRequest,
    checkOutPullRequest,
  ],
  beforeRowClose,
  flags: githubFlags,
  settingsPage: SettingsPage,
});

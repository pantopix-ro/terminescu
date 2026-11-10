// The git plugin's UI half (add-git-plugin group 3): branch and pull request chips on tabs and
// rows, the Changes section, the diff tab and worktree rows. Everything it shows comes from its
// Rust half.

import { definePlugin } from "@terminescu/plugin-api";
import { ChangesSection } from "./ChangesSection";
import { rowChips, tabChips } from "./chips";
import { DIFF_KIND, DiffView } from "./DiffView";
import { loadMode, saveMode, toggled } from "./mode";
import { beforeRowClose, newWorktreeRow, openWorktreeRows } from "./worktree";
import "./git.css";

export { ChangesSection, markOf, type Change, type ChangesAnswer } from "./ChangesSection";
export { MAX_BRANCH, rowChips, shortBranch, shortenMiddle, tabChips } from "./chips";
export { baseNote, DIFF_KIND, DiffView, diffView, parseSource, type BaseText, type DiffSource } from "./DiffView";
export { beforeRowClose, newWorktreeRow, openWorktreeRows } from "./worktree";
export { loadMode, MODE_KEY, parseMode, saveMode, toggled, type DiffMode } from "./mode";

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
    { id: "diffLayout", label: "Toggle Diff Layout", menu: "View", accel: "Cmd+Alt+U", run: () => saveMode(toggled(loadMode())) },
  ],
  beforeRowClose,
});

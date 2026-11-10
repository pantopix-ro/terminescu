// The git plugin's UI half (add-git-plugin group 3): branch and pull request chips on tabs and
// rows, and the Changes section. Everything it shows comes from its Rust half.

import { definePlugin } from "@terminescu/plugin-api";
import { ChangesSection } from "./ChangesSection";
import { rowChips, tabChips } from "./chips";
import "./git.css";

export { ChangesSection, markOf, type Change, type ChangesAnswer } from "./ChangesSection";
export { MAX_BRANCH, rowChips, shortBranch, shortenMiddle, tabChips } from "./chips";

export const git = definePlugin({
  id: "git",
  name: "Git",
  tabDecorators: [tabChips],
  rowDecorators: [rowChips],
  panelSections: [{ id: "changes", title: "Changes", render: ChangesSection }],
});

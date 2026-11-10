// Branch and pull request chips (add-git-plugin D7). The Rust half sets them on tabs; a tab
// shows its own, a one-tab row its tab's, a folder row its shown tab's.

import type { Decoration } from "@terminescu/host";
import { toItems, type Decorator, type RenderItem, type RowView, type TabView } from "@terminescu/plugin-api";

/** A branch name longer than this shortens in the middle. */
export const MAX_BRANCH = 32;

/** Keeps the start and the end of `text` around one `…`, `max` characters in all. */
export function shortenMiddle(text: string, max = MAX_BRANCH): string {
  const chars = Array.from(text);
  if (chars.length <= max) return text;
  const keep = max - 1;
  const head = Math.ceil(keep / 2);
  return `${chars.slice(0, head).join("")}…${chars.slice(chars.length - (keep - head)).join("")}`;
}

/**
 * The branch chip reads `<branch>[ •][ ↑n][ ↓n]`; a branch name has no spaces, so the name is
 * everything before the first one. A shortened name goes whole into the tip.
 */
export function shortBranch(item: RenderItem): RenderItem {
  if (item.key !== "branch") return item;
  const space = item.text.indexOf(" ");
  const name = space < 0 ? item.text : item.text.slice(0, space);
  const short = shortenMiddle(name);
  if (short === name) return item;
  return { ...item, text: short + (space < 0 ? "" : item.text.slice(space)), tip: item.tip ? `${name}\n${item.tip}` : name };
}

const onTab = (d: Decoration, tab: string | undefined) => tab !== undefined && d.target.kind === "tab" && d.target.id === tab;

export const tabChips: Decorator<TabView> = (items, tab, mine) => [...items, ...toItems(mine.filter((d) => onTab(d, tab.id))).map(shortBranch)];

export const rowChips: Decorator<RowView> = (items, row, mine) => {
  const shown = row.tabIds.length === 1 ? row.tabIds[0] : row.shownTabId;
  const own = mine.filter((d) => (d.target.kind === "sideTab" && d.target.id === row.id) || onTab(d, shown));
  return [...items, ...toItems(own).map(shortBranch)];
};

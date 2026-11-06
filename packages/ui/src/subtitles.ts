import { terminalsOf, type PaneLabel, type Tab } from "@terminescu/host";
import type { Agents } from "./agents";

/** Live pane labels by pane id, from the core (task 8.30). */
export type Labels = Readonly<Record<string, PaneLabel>>;

/** Longest path a subtitle shows; a longer one keeps its last folders after `…/`. */
export const MAX_PATH = 28;

export function shortPath(path: string, max = MAX_PATH): string {
  if (path.length <= max) return path;
  const parts = path.split("/");
  let out = parts.pop() ?? path;
  while (parts.length > 0 && parts[parts.length - 1]!.length + out.length + 3 <= max) out = `${parts.pop()}/${out}`;
  return out.length + 2 <= max ? `…/${out}` : `…${out.slice(-(max - 1))}`;
}

/**
 * A pane's live subtitle: its folder, then `claude` and Claude's title while Claude runs there,
 * or the running program: `~/work/api · npm`, `~/work/api · claude · Fix PROJ-482`.
 */
export function paneSubtitle(label: PaneLabel | undefined, claude: boolean): string | undefined {
  const parts: string[] = [];
  if (label?.path) parts.push(shortPath(label.path));
  if (claude) parts.push("claude", ...(label?.title ? [label.title] : []));
  else if (label?.program) parts.push(label.program);
  return parts.length > 0 ? parts.join(" · ") : undefined;
}

/** The pane a tab's subtitle follows: its focused pane when that is a terminal, else its first terminal. */
export function subtitlePane(tab: Tab, focused: string | undefined): string | undefined {
  const terms = terminalsOf(tab.layout);
  return focused && terms.includes(focused) ? focused : terms[0];
}

/** Each tab's subtitle by tab id; a tab with no terminal pane has none. */
export function tabSubtitles(tabs: Iterable<Tab>, focusByTab: Readonly<Record<string, string>>, labels: Labels, agents: Agents): Record<string, string> {
  const out: Record<string, string> = {};
  for (const t of tabs) {
    const pane = subtitlePane(t, focusByTab[t.id]);
    const sub = pane && paneSubtitle(labels[pane], !!agents[pane]);
    if (sub) out[t.id] = sub;
  }
  return out;
}

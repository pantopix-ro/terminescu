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
export function paneSubtitle(label: PaneLabel | undefined, claude: boolean, named = false, session?: string | null): string | undefined {
  const parts: string[] = [];
  if (label?.path) parts.push(shortPath(label.path));
  // A tab named after its program (task 8.62) leaves the program out of its subtitle, and one
  // named after its session (task 8.88) leaves out a title that only repeats that name.
  const title = label?.title && !(named && session && label.title === session) ? [label.title] : [];
  if (claude) parts.push(...(named ? [] : ["claude"]), ...title);
  else if (label?.program && !named) parts.push(label.program);
  return parts.length > 0 ? parts.join(" · ") : undefined;
}

/** The pane a tab's subtitle follows: its focused pane when that is a terminal, else its first terminal. */
export function subtitlePane(tab: Tab, focused: string | undefined): string | undefined {
  const terms = terminalsOf(tab.layout);
  return focused && terms.includes(focused) ? focused : terms[0];
}

/**
 * A terminal pane's name, as Terminal.app names a tab (task 8.62): while Claude runs, its
 * session's name (task 8.88) or `claude`; else the command the shell runs, else the pty's
 * foreground process, `zsh` at the prompt.
 */
export function paneName(label: PaneLabel | undefined, claude: boolean, session?: string | null): string | undefined {
  return claude ? session || "claude" : (label?.program ?? label?.process);
}

/** A tab with its focused terminal's live name; `tabName` uses it after the tab's own title. */
export type LiveTab = Tab & { live?: string };

/** The workspace's side tabs with each tab's live name, from its focused terminal. */
export function withLiveNames<S extends { tabs: Tab[] }>(sides: readonly S[], focusByTab: Readonly<Record<string, string>>, labels: Labels, agents: Agents): S[] {
  return sides.map((s) => ({
    ...s,
    tabs: s.tabs.map((t): LiveTab => {
      const pane = subtitlePane(t, focusByTab[t.id]);
      const live = pane && paneName(labels[pane], !!agents[pane], agents[pane]?.name);
      return live ? { ...t, live } : t;
    }),
  }));
}

/** Each tab's subtitle by tab id; a tab with no terminal pane has none. */
export function tabSubtitles(tabs: Iterable<Tab>, focusByTab: Readonly<Record<string, string>>, labels: Labels, agents: Agents): Record<string, string> {
  const out: Record<string, string> = {};
  for (const t of tabs) {
    const pane = subtitlePane(t, focusByTab[t.id]);
    const sub = pane && paneSubtitle(labels[pane], !!agents[pane], t.title == null, agents[pane]?.name);
    if (sub) out[t.id] = sub;
  }
  return out;
}

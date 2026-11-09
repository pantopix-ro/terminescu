import { dockPlace, panesOf, removePane, rescale, splitAt, type DockSide, type Layout } from "@terminescu/host";

/**
 * The tabs a window shows side by side (tasks 8.71, 8.75): a split tree whose leaves are tab
 * ids, so two tabs of one folder can show at once. The window keeps it; the core never sees it.
 */
export type View = Layout;

export const rowsOf = (v: View | undefined): string[] => (v ? panesOf(v) : []);

const one = (row: string): View => ({ type: "pane", id: row });

/** The view with leaf `from` showing row `to` instead. */
export function replaceRow(v: View, from: string, to: string): View {
  if (v.type === "pane") return v.id === from ? one(to) : v;
  return { ...v, children: v.children.map((c) => replaceRow(c, from, to)) };
}

/** The view without `row`; undefined when it was the last one or never there. */
export function hideRow(v: View | undefined, row: string): View | undefined {
  if (!v) return undefined;
  const out = removePane(v, row);
  return out === undefined ? v : (out ?? undefined);
}

/**
 * The view fitted to the window's tabs: the selected tab, when not shown, takes the place of
 * `prev`, the tab selected before (else the first), even when `prev` just closed; then tabs
 * that left the window leave it.
 */
export function fitView(v: View | undefined, ids: readonly string[], active: string | undefined, prev: string | undefined): View | undefined {
  let out = v;
  const rows = rowsOf(v);
  if (active && out && !rows.includes(active)) out = replaceRow(out, prev && rows.includes(prev) ? prev : rows[0]!, active);
  for (const row of rowsOf(out)) if (!ids.includes(row)) out = hideRow(out, row);
  return out ?? (active ? one(active) : undefined);
}

/**
 * The view with `rows` on `side` of the area showing `target`, sharing half of its share; a
 * folder's tabs come as several (task 8.75). A tab already shown moves there. Undefined when
 * `target` is not shown, is one of `rows`, or is too small to halve.
 */
export function showBeside(v: View | undefined, target: string, rows: readonly string[], side: DockSide, splitId: string): View | undefined {
  if (!rows.length || rows.includes(target)) return undefined;
  const base = rows.reduce<View | undefined>(hideRow, v) ?? one(target);
  const [dir, before] = dockPlace(side);
  const added: View = rows.length === 1 ? one(rows[0]!) : { type: "split", id: `${splitId}-in`, dir, children: rows.map(one), sizes: rescale(rows.map(() => 1)) };
  const out = splitAt(base, target, dir, before, added, splitId);
  return out === "small" ? undefined : out;
}

/**
 * One split tree for the whole window: each leaf of the view replaced by the layout its row
 * shows. A leaf with no layout leaves the tree.
 */
export function composeView(v: View, layoutOf: (row: string) => Layout | undefined): Layout | undefined {
  if (v.type === "pane") return layoutOf(v.id);
  const kept = v.children.map((c, i) => [composeView(c, layoutOf), v.sizes[i]!] as const).filter(([c]) => c);
  if (!kept.length) return undefined;
  if (kept.length === 1) return kept[0]![0];
  return { ...v, children: kept.map(([c]) => c!), sizes: rescale(kept.map(([, s]) => s)) };
}

/** A fresh id for a split of the view; core ids never start with `view-`. */
export const viewSplitId = () => `view-${Math.random().toString(36).slice(2, 10)}`;

/**
 * A folder's tabs for its pixel tip (task 8.74), one line each: the shown one marked `▸`, then
 * its name and its directory, so two tabs both named `zsh` tell apart.
 */
export function folderLines(tabs: readonly { id: string }[], shown: string | undefined, name: (id: string) => string, sub: Readonly<Record<string, string>>): string[] {
  return tabs.map((t) => [`${t.id === shown ? "▸" : " "} ${name(t.id)}`, sub[t.id]].filter(Boolean).join("  "));
}

import { AUTOS, TINTS, type Auto, type Look, type Shown, type SideTab, type StatusConfig } from "@terminescu/host";
import type { CSSProperties } from "react";

/** The built-in looks before any rename or recolor in Settings (design board "Settings"). */
export const BUILTIN_LOOKS: Readonly<Record<Auto, Look>> = {
  waiting: { label: "Waiting", accent: null, background: null },
  running: { label: "Running", accent: "blue", background: null },
  done: { label: "Done", accent: "green", background: null },
  failed: { label: "Failed", accent: "red", background: null },
  idle: { label: "Idle", accent: null, background: null },
};

export type Builtins = StatusConfig["builtins"];

export const builtinLook = (auto: Auto, builtins: Builtins = {}): Look => builtins[auto] ?? BUILTIN_LOOKS[auto];

/** What a status chip shows. */
export interface Chip extends Look {
  /** Set by hand, a rule or the CLI: the chip carries the small square. */
  set: boolean;
  /** Amber chip: the auto status Waiting shows. A set status keeps its colors; the row or tab
   * around it carries the amber marker instead. */
  need: boolean;
}

export function chipOf(shown: Shown | undefined, builtins: Builtins = {}): Chip {
  if (!shown) return { ...builtinLook("idle", builtins), set: false, need: false };
  if (shown.look) return { ...shown.look, set: true, need: false };
  return { ...builtinLook(shown.auto, builtins), set: false, need: shown.auto === "waiting" };
}

const hue = (c: string) => (c === "fg" ? "var(--fg)" : `var(--${c})`);

/** CSS variables for an `ao-stat` chip. Amber chips take theirs from `.ao-stat.need`. */
export function chipStyle(c: Look & { need?: boolean }): CSSProperties | undefined {
  if (c.need) return undefined;
  const style: Record<string, string> = {};
  if (c.accent) style["--s-fg"] = style["--s-edge"] = hue(c.accent);
  if (c.background) style["--s-bg"] = `var(--tint-${c.background})`;
  return style as CSSProperties;
}

/** Accent and background variables for a sidebar row (`--st-*`) or a tab (`--tab-*`). */
export function rowStyle(look: Look | undefined, prefix: "st" | "tab"): Record<string, string> {
  const style: Record<string, string> = {};
  if (look?.accent) style[`--${prefix}-accent`] = hue(look.accent);
  if (look?.background) style[`--${prefix}-bg`] = `var(--tint-${look.background})`;
  return style;
}

export const SORTS = ["manual", "background", "status", "name"] as const;
export type SortMode = (typeof SORTS)[number];

const urgency = (s: Shown | undefined) => (s?.waiting ? AUTOS.length : AUTOS.indexOf(s?.auto ?? "idle"));

/**
 * The sidebar order (task 5.2). Manual keeps the saved order. Background groups equal tints in
 * the tint order, untinted last, by name inside each. Status puts the most urgent first, by
 * name inside. Name sorts by name.
 */
export function sortSideTabs(
  sides: readonly SideTab[],
  statuses: Readonly<Record<string, Shown>>,
  mode: SortMode,
  name: (s: SideTab) => string,
): SideTab[] {
  const byName = (a: SideTab, b: SideTab) => name(a).localeCompare(name(b), undefined, { sensitivity: "base" });
  const list = [...sides];
  if (mode === "manual") return list;
  if (mode === "name") return list.sort(byName);
  if (mode === "status") return list.sort((a, b) => urgency(statuses[b.id]) - urgency(statuses[a.id]) || byName(a, b));
  const tint = (s: SideTab) => {
    const bg = statuses[s.id]?.look?.background;
    return bg ? TINTS.indexOf(bg) : TINTS.length;
  };
  return list.sort((a, b) => tint(a) - tint(b) || byName(a, b));
}

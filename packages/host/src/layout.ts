// The split tree, as in crates/core/src/layout.rs. The core plans every change; this copy serves
// the mock host and the UI's lookups.

import type { Dir, DockSide, Id, Layout } from "./protocol";

export const TOTAL = 1000;
export const MIN_SIZE = 50;

/** Pane ids in reading order. */
export function panesOf(l: Layout): Id[] {
  return l.type === "pane" ? [l.id] : l.children.flatMap(panesOf);
}

export function hasSplit(l: Layout, id: Id): boolean {
  return l.type === "split" && (l.id === id || l.children.some((c) => hasSplit(c, id)));
}

export function leaf(l: Layout, id: Id): Extract<Layout, { type: "pane" }> | undefined {
  if (l.type === "pane") return l.id === id ? l : undefined;
  for (const c of l.children) {
    const found = leaf(c, id);
    if (found) return found;
  }
  return undefined;
}

/** Weights to sizes adding up to exactly TOTAL, each at least 1. */
export function rescale(weights: number[]): number[] {
  const sum = weights.reduce((a, b) => a + b, 0);
  const out =
    sum === 0
      ? weights.map(() => Math.floor(TOTAL / Math.max(weights.length, 1)))
      : weights.map((w) => Math.max(1, Math.floor((w * TOTAL) / sum)));
  const diff = TOTAL - out.reduce((a, b) => a + b, 0);
  let big = 0;
  // The last of the largest shares takes the rounding difference, as in layout.rs.
  out.forEach((s, i) => s >= out[big]! && (big = i));
  if (out.length) out[big]! += diff;
  return out;
}

/**
 * The tree with `added` placed after `target`: undefined when `target` is unknown, "small"
 * when the split would leave a share under MIN_SIZE.
 */
export function splitPane(
  l: Layout,
  target: Id,
  dir: Dir,
  added: Id,
  splitId: Id,
  extra: { transcript?: string; doc?: Id } = {},
): Layout | undefined | "small" {
  return splitAt(l, target, dir, false, { type: "pane", id: added, ...extra }, splitId);
}

/** A dock side's split direction, and whether the tab goes before the pane. */
export function dockPlace(side: DockSide): [Dir, boolean] {
  return [side === "left" || side === "right" ? "row" : "column", side === "left" || side === "up"];
}

/**
 * The tree with `added`, a pane or a whole tab's layout, before or after `target`, taking half
 * its share, as `split_at` in layout.rs: a split `added` in the same direction merges into its
 * new parent. Undefined when `target` is unknown, "small" when a share would end under MIN_SIZE.
 */
export function splitAt(l: Layout, target: Id, dir: Dir, before: boolean, added: Layout, splitId: Id): Layout | undefined | "small" {
  let small = false;
  const go = (node: Layout): Layout | undefined => {
    if (node.type === "pane") {
      if (node.id !== target) return undefined;
      return before
        ? { type: "split", id: splitId, dir, children: [added, node], sizes: [TOTAL / 2, TOTAL - TOTAL / 2] }
        : { type: "split", id: splitId, dir, children: [node, added], sizes: [TOTAL - TOTAL / 2, TOTAL / 2] };
    }
    const i = node.children.findIndex((c) => c.type === "pane" && c.id === target);
    if (i >= 0 && node.dir === dir) {
      if (node.sizes[i]! < 2 * MIN_SIZE) {
        small = true;
        return undefined;
      }
      const sizes = [...node.sizes];
      const half = Math.floor(sizes[i]! / 2);
      sizes[i]! -= half;
      const at = before ? i : i + 1;
      sizes.splice(at, 0, half);
      const children = [...node.children];
      children.splice(at, 0, added);
      return { ...node, children, sizes };
    }
    for (let k = 0; k < node.children.length && !small; k++) {
      const placed = go(node.children[k]!);
      if (placed) {
        const children = [...node.children];
        children[k] = placed;
        return { ...node, children };
      }
    }
    return undefined;
  };
  let out = go(structuredClone(l));
  if (small) return "small";
  if (out && added.type === "split") {
    out = normalize(out);
    if (!sharesFit(out)) return "small";
  }
  return out;
}

function sharesFit(l: Layout): boolean {
  return l.type === "pane" || (l.sizes.every((s) => s >= MIN_SIZE) && l.children.every(sharesFit));
}

/** The tree without `target`: null when it was the last pane, undefined when it is unknown. */
export function removePane(l: Layout, target: Id): Layout | null | undefined {
  if (!panesOf(l).includes(target)) return undefined;
  if (l.type === "pane") return null;
  const next = structuredClone(l);
  const cut = (node: Layout) => {
    if (node.type === "pane") return;
    const i = node.children.findIndex((c) => c.type === "pane" && c.id === target);
    if (i >= 0) {
      node.children.splice(i, 1);
      const [freed] = node.sizes.splice(i, 1);
      node.sizes[Math.min(Math.max(i - 1, 0), node.sizes.length - 1)]! += freed!;
      return;
    }
    node.children.forEach(cut);
  };
  cut(next);
  return normalize(next);
}

function normalize(node: Layout): Layout {
  if (node.type === "pane") return node;
  const children: Layout[] = [];
  const weights: number[] = [];
  node.children.map(normalize).forEach((c, i) => {
    const s = node.sizes[i]!;
    if (c.type === "split" && c.dir === node.dir) {
      c.children.forEach((g, j) => {
        children.push(g);
        weights.push(s * c.sizes[j]!);
      });
    } else {
      children.push(c);
      weights.push(s * TOTAL);
    }
  });
  if (children.length === 1) return children[0]!;
  return { ...node, children, sizes: rescale(weights) };
}

export function resizeSplit(l: Layout, splitId: Id, sizes: number[]): Layout | "unknown" | "bad" {
  if (!hasSplit(l, splitId)) return "unknown";
  const next = structuredClone(l);
  let bad = false as boolean;
  const walk = (node: Layout) => {
    if (node.type === "pane") return;
    if (node.id === splitId) {
      const scaled = sizes.length === node.children.length ? rescale(sizes) : [];
      if (!scaled.length || scaled.some((s) => s < MIN_SIZE)) bad = true;
      else node.sizes = scaled;
      return;
    }
    node.children.forEach(walk);
  };
  walk(next);
  return bad ? "bad" : next;
}

/** A Claude session id safe to type after `claude --resume`, as `is_session_id` in runtime.rs. */
export function isSessionId(s: string): boolean {
  return /^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$/.test(s);
}

/** The session a transcript pane mirrors, or undefined for a terminal pane. */
export function transcriptOf(l: Layout, id: Id): string | undefined {
  return leaf(l, id)?.transcript;
}

/** The document a text pane edits, or undefined for other panes. */
export function docOf(l: Layout, id: Id): Id | undefined {
  return leaf(l, id)?.doc;
}

/** Pane ids that run a shell, in reading order: transcript and text panes left out. */
export function terminalsOf(l: Layout): Id[] {
  return panesOf(l).filter((p) => !transcriptOf(l, p) && !docOf(l, p));
}

/** A tab stays open while it has a shell or a document; transcripts alone close it. */
export function holdsWork(l: Layout): boolean {
  return panesOf(l).some((p) => !transcriptOf(l, p));
}

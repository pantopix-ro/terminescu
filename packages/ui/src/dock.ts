import type { DockSide } from "@terminescu/host";

/** A pane's box on screen, as `getBoundingClientRect` gives it. */
export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** Share of a pane's width and height, around its middle, where a dropped tab does nothing. */
export const DOCK_CENTER = 0.3;

/**
 * Where a tab dropped at a point lands on a pane (task 8.32): the diagonals cut the pane into
 * four triangles, one per side; the middle square cancels. Undefined outside the pane.
 */
export function dockZone(box: Box, x: number, y: number): DockSide | "center" | undefined {
  if (box.width <= 0 || box.height <= 0) return undefined;
  const dx = (x - box.left) / box.width - 0.5;
  const dy = (y - box.top) / box.height - 0.5;
  if (Math.abs(dx) > 0.5 || Math.abs(dy) > 0.5) return undefined;
  if (Math.abs(dx) < DOCK_CENTER / 2 && Math.abs(dy) < DOCK_CENTER / 2) return "center";
  if (Math.abs(dx) >= Math.abs(dy)) return dx > 0 ? "right" : "left";
  return dy > 0 ? "down" : "up";
}

/** The half of the pane the dropped tab takes. */
export function dockBox(box: Box, side: DockSide): Box {
  const w = box.width / 2;
  const h = box.height / 2;
  switch (side) {
    case "left":
      return { ...box, width: w };
    case "right":
      return { ...box, left: box.left + w, width: w };
    case "up":
      return { ...box, height: h };
    case "down":
      return { ...box, top: box.top + h, height: h };
  }
}

/**
 * WebKit keeps 16 live WebGL contexts per window and silently drops the oldest past that
 * (spike 0.4). Panes take a context only while visible, and at most 12 at once, leaving room
 * for anything else on the page (D33).
 */
export const WEBGL_LIMIT = 12;

let live = 0;

export const webglBudget = {
  acquire(): boolean {
    if (live >= WEBGL_LIMIT) return false;
    live++;
    return true;
  },
  release(): void {
    live = Math.max(0, live - 1);
  },
  get live(): number {
    return live;
  },
};

// One DOM element per pane, created on first use. The xterm.js terminal lives inside it, and
// the split view moves the element between slots, so a split or a tab move never rebuilds the
// terminal (D33).

const elements = new Map<string, HTMLDivElement>();
let pool: HTMLDivElement | undefined;

/** A hidden holder for panes that no slot shows, so their terminals stay in the document. */
function poolElement(): HTMLDivElement {
  if (!pool || !pool.isConnected) {
    pool = document.createElement("div");
    pool.className = "pane-pool";
    pool.hidden = true;
    document.body.append(pool);
  }
  return pool;
}

export function paneElement(paneId: string): HTMLDivElement {
  let el = elements.get(paneId);
  if (!el) {
    el = document.createElement("div");
    el.className = "term-pane";
    el.dataset.pane = paneId;
    el.hidden = true;
    poolElement().append(el);
    elements.set(paneId, el);
  }
  return el;
}

/** Puts a pane's element back in the pool, unless another slot took it already. */
export function parkPane(paneId: string, from: HTMLElement) {
  const el = elements.get(paneId);
  if (el && el.parentElement === from) poolElement().append(el);
}

const claims = new Map<string, number>();

/** A terminal holds the pane's element while it lives. */
export function claimPane(paneId: string): HTMLDivElement {
  claims.set(paneId, (claims.get(paneId) ?? 0) + 1);
  return paneElement(paneId);
}

/**
 * Lets go of the element. It goes away on the next task unless a new terminal claimed it
 * meanwhile, as React's strict mode does when it remounts an effect.
 */
export function releasePane(paneId: string) {
  const left = (claims.get(paneId) ?? 1) - 1;
  if (left > 0) return void claims.set(paneId, left);
  claims.delete(paneId);
  setTimeout(() => {
    if (claims.has(paneId)) return;
    elements.get(paneId)?.remove();
    elements.delete(paneId);
  });
}

/** Gives the pane's terminal the keyboard. */
export function focusPane(paneId: string) {
  const term = elements.get(paneId)?.querySelector<HTMLTextAreaElement>("textarea");
  if (term) return term.focus();
  // A transcript pane has no terminal; its root takes the focus.
  document.querySelector<HTMLElement>(`[data-transcript="${CSS.escape(paneId)}"]`)?.focus();
}

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

/** Gives the pane's terminal or editor the keyboard. */
/**
 * A name edited in place or an open icon picker keeps the keyboard: the click that selected
 * the row queued a pane focus before the double click opened them.
 */
export const keyboardHeld = () => !!document.activeElement?.closest(".side-rename, .icon-picker");

export function focusPane(paneId: string) {
  if (keyboardHeld()) return;
  const el = elements.get(paneId);
  const target = el?.querySelector<HTMLElement>(".cm-content") ?? el?.querySelector<HTMLTextAreaElement>("textarea");
  if (target) return target.focus();
  // A transcript pane has no terminal; its root takes the focus.
  document.querySelector<HTMLElement>(`[data-transcript="${CSS.escape(paneId)}"]`)?.focus();
}

/**
 * Sends a DOM event to a pane: the transcript pane's root, or the terminal's element. Returns
 * whether the pane exists.
 */
export function paneEvent(paneId: string, event: Event): boolean {
  const target =
    document.querySelector<HTMLElement>(`[data-transcript="${CSS.escape(paneId)}"]`) ?? elements.get(paneId);
  if (!target) return false;
  target.dispatchEvent(event);
  return true;
}

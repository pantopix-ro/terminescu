// Every modal dialog sits in a `.sheet-back` backdrop. While one shows, Tab and Shift+Tab
// cycle inside the top one and focus that lands elsewhere goes back into it, so neither the
// keyboard nor a screen reader reaches the panes behind. When the last one closes and focus
// fell to the page, it returns to where it was before the dialog opened.

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"]), [contenteditable="true"]';

/** The dialog in the top backdrop, or null when none shows. */
export function topModal(doc: Document = document): HTMLElement | null {
  const backs = doc.querySelectorAll<HTMLElement>(".sheet-back");
  const back = backs[backs.length - 1];
  return back?.querySelector<HTMLElement>('[role="dialog"], [role="alertdialog"]') ?? null;
}

/** The elements Tab stops at inside `root`, in document order. */
export function tabStops(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.getClientRects().length > 0);
}

/** Where Tab goes from `from` among `stops`: wraps at either end, enters at the edge. */
export function nextStop<T>(stops: T[], from: T | null, back: boolean): T | undefined {
  if (!stops.length) return undefined;
  const at = from === null ? -1 : stops.indexOf(from);
  if (at < 0) return back ? stops[stops.length - 1] : stops[0];
  return stops[(at + (back ? -1 : 1) + stops.length) % stops.length];
}

/** Installs the trap on `root`, the element whose children hold the backdrops. */
export function trapModalFocus(root: HTMLElement): () => void {
  const doc = root.ownerDocument;
  let before: HTMLElement | null = null;
  let open = false;

  const enter = (modal: HTMLElement) => {
    const to = tabStops(modal)[0] ?? modal;
    if (to === modal && !modal.hasAttribute("tabindex")) modal.tabIndex = -1;
    to.focus();
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key !== "Tab" || e.metaKey || e.ctrlKey || e.altKey) return;
    const modal = topModal(doc);
    if (!modal) return;
    const active = doc.activeElement instanceof HTMLElement && modal.contains(doc.activeElement) ? doc.activeElement : null;
    const to = nextStop(tabStops(modal), active, e.shiftKey);
    e.preventDefault();
    e.stopPropagation();
    if (to) to.focus();
    else enter(modal);
  };
  const onFocus = (e: FocusEvent) => {
    const modal = topModal(doc);
    const target = e.target instanceof HTMLElement ? e.target : null;
    if (!modal) {
      if (target) before = target;
      return;
    }
    // A popover over the dialog (a menu, the icon picker) keeps the focus it takes.
    if (target && (modal.contains(target) || !target.closest(".term-pane, .text-pane, .app-side, .ctx-pinned, main"))) return;
    enter(modal);
  };
  const watch = new MutationObserver(() => {
    const modal = topModal(doc);
    if (modal && !open) {
      open = true;
      if (!(doc.activeElement instanceof HTMLElement && modal.contains(doc.activeElement))) enter(modal);
    } else if (!modal && open) {
      open = false;
      const lost = !doc.activeElement || doc.activeElement === doc.body;
      if (lost && before?.isConnected) before.focus();
    }
  });
  // Backdrops are direct children of `root`: watching only those keeps terminal redraws out.
  watch.observe(root, { childList: true });
  doc.addEventListener("keydown", onKey, true);
  doc.addEventListener("focusin", onFocus, true);
  return () => {
    watch.disconnect();
    doc.removeEventListener("keydown", onKey, true);
    doc.removeEventListener("focusin", onFocus, true);
  };
}

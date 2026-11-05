import { useLayoutEffect, useRef, useState } from "react";

const GAP = 4;

/**
 * A small modal list at a window point (icon picker, quick status): kept inside the window,
 * the first of the `focus` selectors that matches takes the keyboard, a pane that grabs the focus as its tab shows
 * hands it straight back, and the focus returns where it was on close.
 */
export function usePopover(x: number, y: number, focus: string[]) {
  const ref = useRef<HTMLDivElement>(null);
  const before = useRef<Element | null>(null);
  const [pos, setPos] = useState<{ left: number; top: number }>();
  const first = () => focus.map((q) => ref.current?.querySelector<HTMLElement>(q)).find(Boolean);
  useLayoutEffect(() => {
    before.current = document.activeElement;
    const el = ref.current!;
    const left = Math.max(GAP, Math.min(x, window.innerWidth - GAP - el.offsetWidth));
    const top = Math.max(GAP, Math.min(y, window.innerHeight - GAP - el.offsetHeight));
    setPos({ left, top });
    const keep = (e: FocusEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) first()?.focus();
    };
    document.addEventListener("focusin", keep);
    return () => {
      document.removeEventListener("focusin", keep);
      const back = before.current;
      if (back instanceof HTMLElement && back.isConnected) back.focus({ preventScroll: true });
    };
  }, [x, y]);
  // A hidden element takes no focus, so the focus moves once the list has its place.
  const placed = !!pos;
  useLayoutEffect(() => {
    if (placed) first()?.focus();
  }, [placed]);
  return { ref, style: pos ?? { left: x, top: y, visibility: "hidden" as const } };
}

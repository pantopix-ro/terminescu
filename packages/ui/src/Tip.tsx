import { useLayoutEffect, useRef, useState } from "react";

/** A tooltip's text and the element it points at. */
export interface TipAt {
  name: string;
  /** A second line in plain text, such as a tab's path and program. */
  sub?: string;
  /** The status label, shown under the name. */
  status?: string;
  /** More plain lines under the rest, one per item, as a folder's tabs (task 8.74). */
  lines?: readonly string[];
  rect: DOMRect;
  /** Lay it over the element, as a name's expansion, instead of beside it. */
  over?: boolean;
  /** Put it under the element, as for a tab in the tab bar (task 8.48). */
  below?: boolean;
}

/** A name ellipsised by its box: the custom tooltip shows it whole. */
export const truncated = (el: HTMLElement) => el.scrollWidth > el.clientWidth;

/** The tooltip's state: show it for an element, hide it. */
export function useTip() {
  const [tip, setTip] = useState<TipAt>();
  return {
    tip,
    show: (el: Element, name: string, status?: string) => setTip({ name, status, rect: el.getBoundingClientRect() }),
    /** Under the element: the name, a plain second line and the status. */
    below: (el: Element, name: string, sub?: string, status?: string, lines?: readonly string[]) =>
      setTip({ name, sub, status, lines, rect: el.getBoundingClientRect(), below: true }),
    /** The whole of a name its box cuts short, laid over it; nothing when it fits. */
    expand: (el: HTMLElement, name: string) => {
      if (!truncated(el)) return false;
      setTip({ name, rect: el.getBoundingClientRect(), over: true });
      return true;
    },
    hide: () => setTip(undefined),
  };
}

/**
 * A pixel tooltip beside an element (task 8.23): square, 2 px frame, hard shadow. It sits right
 * of the element, kept inside the window, and never takes the pointer.
 */
export function Tip({ tip }: { tip: TipAt | undefined }) {
  const ref = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<{ left: number; top: number }>();
  useLayoutEffect(() => {
    const el = ref.current;
    if (!tip || !el) return setAt(undefined);
    const { rect } = tip;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    if (tip.below) {
      const left = Math.max(4, Math.min(rect.left, window.innerWidth - 4 - w));
      const top = rect.bottom + 6 + h > window.innerHeight - 4 ? Math.max(4, rect.top - h - 6) : rect.bottom + 6;
      return setAt({ left, top });
    }
    // Over: the tooltip's text lines up with the name under it (6 px padding; the frame is a shadow).
    let left = tip.over ? rect.left - 6 : rect.right + 8;
    if (left + w > window.innerWidth - 4) left = tip.over ? window.innerWidth - 4 - w : rect.left - w - 8;
    left = Math.max(4, left);
    const top = Math.min(Math.max(4, rect.top + rect.height / 2 - h / 2), window.innerHeight - h - 4);
    setAt({ left, top });
  }, [tip]);
  if (!tip) return null;
  return (
    <div
      ref={ref}
      role="tooltip"
      className={`px-tip${tip.over ? " over" : ""}`}
      style={at ? { left: at.left, top: at.top } : { left: 0, top: 0, visibility: "hidden" }}
    >
      <span className="px-tip-name">{tip.name}</span>
      {tip.sub && <span className="px-tip-sub">{tip.sub}</span>}
      {tip.status && <span className="px-tip-status">{tip.status}</span>}
      {tip.lines?.map((l, i) => (
        <span key={i} className="px-tip-sub px-tip-line">
          {l}
        </span>
      ))}
    </div>
  );
}

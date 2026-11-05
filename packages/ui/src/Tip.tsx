import { useLayoutEffect, useRef, useState } from "react";

/** A tooltip's text and the element it points at. */
export interface TipAt {
  name: string;
  /** The status label, shown under the name. */
  status?: string;
  rect: DOMRect;
  /** Lay it over the element, as a name's expansion, instead of beside it. */
  over?: boolean;
}

/** A name ellipsised by its box: the custom tooltip shows it whole. */
export const truncated = (el: HTMLElement) => el.scrollWidth > el.clientWidth;

/** The tooltip's state: show it for an element, hide it. */
export function useTip() {
  const [tip, setTip] = useState<TipAt>();
  return {
    tip,
    show: (el: Element, name: string, status?: string) => setTip({ name, status, rect: el.getBoundingClientRect() }),
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
      {tip.status && <span className="px-tip-status">{tip.status}</span>}
    </div>
  );
}

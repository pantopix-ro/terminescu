import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";

/** The row a shell picker belongs to, and where that row sits. */
export interface PickAt {
  sideId: string;
  rect: DOMRect;
}

/** How long the pointer rests on a row before its shells show, and how long the list waits for it to cross over. */
export const PICK_OPEN_MS = 250;
export const PICK_CLOSE_MS = 160;

/** The picker's state: a hover opens it after a rest, leaving both the row and the list closes it. */
export function usePicker() {
  const [at, setAt] = useState<PickAt>();
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  const later = (ms: number, f: () => void) => {
    clearTimeout(timer.current);
    timer.current = setTimeout(f, ms);
  };
  return {
    at,
    enter: (sideId: string, el: Element) => {
      const rect = el.getBoundingClientRect();
      // Another row's list is already open: move at once.
      if (at) {
        clearTimeout(timer.current);
        setAt({ sideId, rect });
      } else later(PICK_OPEN_MS, () => setAt({ sideId, rect }));
    },
    leave: () => later(PICK_CLOSE_MS, () => setAt(undefined)),
    keep: () => clearTimeout(timer.current),
    close: () => {
      clearTimeout(timer.current);
      setAt(undefined);
    },
  };
}

/**
 * A row's shells in a pixel popover beside it, over the panes (task 8.28): one click on a shell
 * opens it, whichever row is active.
 */
export function ShellPicker(props: { at: PickAt; label: string; onEnter(): void; onLeave(): void; children: ReactNode }) {
  const { at } = props;
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number }>();
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const h = el.offsetHeight;
    const left = Math.min(at.rect.right + 4, window.innerWidth - el.offsetWidth - 4);
    const top = Math.max(4, Math.min(at.rect.top, window.innerHeight - h - 4));
    setPos({ left, top });
  }, [at]);
  return (
    <div
      ref={ref}
      role="group"
      aria-label={props.label}
      className="px-tip px-pick"
      style={pos ?? { left: 0, top: 0, visibility: "hidden" }}
      onMouseEnter={props.onEnter}
      onMouseLeave={props.onLeave}
    >
      {props.children}
    </div>
  );
}

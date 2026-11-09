import { MIN_SIZE, TOTAL, type Layout } from "@terminescu/host";
import { Fragment, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { paneElement, parkPane } from "./pane-registry";

export interface SplitViewProps {
  layout: Layout;
  /**
   * Draws the frame around one pane; the terminal goes into `slot`. A transcript pane gets the
   * session it mirrors and no slot (task 4.4).
   */
  renderPane(paneId: string, resume: string | undefined, slot: ReactNode, transcript?: string): ReactNode;
  /** Resolves `false` when the resize was refused, so the preview snaps back. */
  onResize(splitId: string, sizes: number[]): void | Promise<boolean>;
  /**
   * Wraps the subtree under the node `id`, a pane or a split, or returns `body` as is. A
   * window showing several rows heads each row's area this way (task 8.73).
   */
  frame?(id: string, body: ReactNode): ReactNode;
}

/** Where a pane's terminal shows. The element moves in and out, the terminal stays (D33). */
export function PaneSlot({ paneId }: { paneId: string }) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const slot = ref.current!;
    slot.append(paneElement(paneId));
    return () => parkPane(paneId, slot);
  }, [paneId]);
  return <div ref={ref} className="pane-slot" />;
}

/** Moves the border between children `i` and `i + 1` by `delta` thousandths, both kept at MIN_SIZE. */
export function dragSizes(sizes: number[], i: number, delta: number): number[] {
  const out = [...sizes];
  const pair = out[i]! + out[i + 1]!;
  const a = Math.min(Math.max(Math.round(out[i]! + delta), MIN_SIZE), pair - MIN_SIZE);
  out[i] = a;
  out[i + 1] = pair - a;
  return out;
}

function Split({ node, renderPane, onResize, frame }: { node: Extract<Layout, { type: "split" }> } & Omit<SplitViewProps, "layout">) {
  const ref = useRef<HTMLDivElement>(null);
  const [preview, setPreview] = useState<number[] | null>(null);
  useEffect(() => setPreview(null), [node.sizes]);
  const sizes = preview ?? node.sizes;
  const row = node.dir === "row";

  const startDrag = (i: number, e: React.PointerEvent<HTMLDivElement>) => {
    const box = ref.current!.getBoundingClientRect();
    const length = row ? box.width : box.height;
    if (length <= 0) return;
    const start = row ? e.clientX : e.clientY;
    const base = node.sizes;
    let last = base;
    const bar = e.currentTarget;
    bar.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      last = dragSizes(base, i, (((row ? ev.clientX : ev.clientY) - start) * TOTAL) / length);
      setPreview(last);
    };
    const end = (ev: PointerEvent) => {
      bar.removeEventListener("pointermove", move);
      bar.removeEventListener("pointerup", end);
      bar.removeEventListener("pointercancel", end);
      if (ev.type === "pointerup" && last !== base) {
        void Promise.resolve(onResize(node.id, last)).then((ok) => ok === false && setPreview(null));
      } else setPreview(null);
    };
    bar.addEventListener("pointermove", move);
    bar.addEventListener("pointerup", end);
    bar.addEventListener("pointercancel", end);
  };

  return (
    <div ref={ref} className={`split split-${node.dir}`} data-split={node.id}>
      {node.children.map((child, i) => (
        <Fragment key={child.id}>
          {i > 0 && (
            <div
              className="split-bar"
              role="separator"
              aria-orientation={row ? "vertical" : "horizontal"}
              onPointerDown={(e) => startDrag(i - 1, e)}
            />
          )}
          <div className="split-child" style={{ flex: `${sizes[i]!} 1 0px` }}>
            <Node node={child} renderPane={renderPane} onResize={onResize} frame={frame} />
          </div>
        </Fragment>
      ))}
    </div>
  );
}

function Node({ node, renderPane, onResize, frame }: { node: Layout } & Omit<SplitViewProps, "layout">) {
  const body =
    node.type === "split" ? (
      <Split node={node} renderPane={renderPane} onResize={onResize} frame={frame} />
    ) : node.transcript ? (
      renderPane(node.id, undefined, null, node.transcript)
    ) : (
      renderPane(node.id, node.resume, <PaneSlot paneId={node.id} />)
    );
  return <>{frame ? frame(node.id, body) : body}</>;
}

/** A tab's split tree as nested flex boxes with draggable borders (D3, D19). */
export function SplitView({ layout, renderPane, onResize, frame }: SplitViewProps) {
  return (
    <div className="split-root">
      <Node node={layout} renderPane={renderPane} onResize={onResize} frame={frame} />
    </div>
  );
}

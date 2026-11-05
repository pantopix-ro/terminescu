import { useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";

export type MenuEntry =
  | {
      id: string;
      label: string;
      /** A chord shown on the right, as the palette shows it. */
      hint?: string;
      danger?: boolean;
      disabled?: boolean;
      run?(): void;
      /** A submenu, opened with a hover, a click or the right arrow. */
      items?: MenuEntry[];
    }
  | "-";

export interface ContextMenuProps {
  /** Where the right click landed, in window pixels. */
  x: number;
  y: number;
  items: MenuEntry[];
  /** Says what the menu is for, for screen readers. */
  label: string;
  onClose(): void;
}

const GAP = 4;

/** Keeps a box of `w` × `h` at (`x`, `y`) inside the window, flipping left or up when needed. */
function place(x: number, y: number, w: number, h: number, flipFrom?: number): { left: number; top: number } {
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  let left = x;
  if (left + w > vw - GAP) left = flipFrom !== undefined ? flipFrom - w : vw - GAP - w;
  let top = y;
  if (top + h > vh - GAP) top = Math.max(GAP, vh - GAP - h);
  return { left: Math.max(GAP, left), top: Math.max(GAP, top) };
}

function Panel({
  x,
  y,
  flipFrom,
  items,
  label,
  depth,
  focusFirst = true,
  onClose,
  onBack,
}: {
  x: number;
  y: number;
  flipFrom?: number;
  items: MenuEntry[];
  label: string;
  depth: number;
  /** Opened from the keyboard or a click: the first item takes the focus. A hover leaves it. */
  focusFirst?: boolean;
  onClose(): void;
  onBack?(): void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number }>();
  const [open, setOpen] = useState<{ id: string; x: number; y: number; flip: number; focus: boolean }>();
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    setPos(place(x, y, el.offsetWidth, el.offsetHeight, flipFrom));
  }, [x, y, flipFrom]);
  useLayoutEffect(() => {
    if (pos && focusFirst) ref.current?.querySelector<HTMLElement>("[role=menuitem]:not([aria-disabled=true])")?.focus();
  }, [pos, focusFirst]);

  const buttons = () => [...(ref.current?.querySelectorAll<HTMLElement>(":scope > [role=menuitem]:not([aria-disabled=true])") ?? [])];
  const move = (by: number) => {
    const all = buttons();
    const at = all.indexOf(document.activeElement as HTMLElement);
    all[(at + by + all.length) % all.length]?.focus();
  };
  const openSub = (id: string, el: HTMLElement, focus: boolean) => {
    const r = el.getBoundingClientRect();
    setOpen({ id, x: r.right, y: r.top - 4, flip: r.left, focus });
  };
  const onKeyDown = (e: KeyboardEvent) => {
    // The menu owns the keyboard while it is open; nothing reaches the pane under it.
    e.stopPropagation();
    if (e.key === "ArrowDown") move(1);
    else if (e.key === "ArrowUp") move(-1);
    else if (e.key === "Home") buttons()[0]?.focus();
    else if (e.key === "End") buttons().at(-1)?.focus();
    else if (e.key === "Escape") return depth ? onBack?.() : onClose();
    else if (e.key === "ArrowLeft" && depth) onBack?.();
    else if (e.key === "Tab") onClose();
    else return;
    e.preventDefault();
  };

  return (
    <div
      ref={ref}
      className="ao-frame ctx-menu"
      role="menu"
      aria-label={label}
      style={pos ? { left: pos.left, top: pos.top } : { left: x, top: y, visibility: "hidden" }}
      onKeyDown={onKeyDown}
      onContextMenu={(e) => e.preventDefault()}
    >
      {items.map((it, i) =>
        it === "-" ? (
          <div key={`sep${i}`} className="ctx-sep" role="separator" />
        ) : (
          <div
            key={it.id}
            role="menuitem"
            tabIndex={-1}
            data-menu-item={it.id}
            aria-disabled={it.disabled || undefined}
            aria-haspopup={it.items ? "menu" : undefined}
            aria-expanded={it.items ? open?.id === it.id : undefined}
            className={`ctx-item${it.danger ? " danger" : ""}${open?.id === it.id ? " open" : ""}`}
            onPointerEnter={(e) => {
              if (it.disabled) return;
              e.currentTarget.focus();
              if (it.items) openSub(it.id, e.currentTarget, false);
              else setOpen(undefined);
            }}
            onClick={(e) => {
              if (it.disabled) return;
              if (it.items) return openSub(it.id, e.currentTarget, true);
              onClose();
              it.run?.();
            }}
            onKeyDown={(e) => {
              if (it.disabled) return;
              if (it.items && (e.key === "ArrowRight" || e.key === "Enter" || e.key === " ")) {
                e.preventDefault();
                e.stopPropagation();
                openSub(it.id, e.currentTarget, true);
              } else if (!it.items && (e.key === "Enter" || e.key === " ")) {
                e.preventDefault();
                e.stopPropagation();
                onClose();
                it.run?.();
              }
            }}
          >
            <span className="grow">{it.label}</span>
            {it.hint && <span className="ctx-hint">{it.hint}</span>}
            {it.items && <i className="ao-i x1 ao-i-chevr" aria-hidden="true" />}
          </div>
        ),
      )}
      {open &&
        (() => {
          const parent = items.find((it): it is Exclude<MenuEntry, "-"> => it !== "-" && it.id === open.id);
          if (!parent?.items) return null;
          return (
            <Panel
              key={open.id}
              x={open.x}
              y={open.y}
              flipFrom={open.flip}
              items={parent.items}
              label={parent.label}
              depth={depth + 1}
              focusFirst={open.focus}
              onClose={onClose}
              onBack={() => {
                setOpen(undefined);
                ref.current?.querySelector<HTMLElement>(`[data-menu-item="${CSS.escape(open.id)}"]`)?.focus();
              }}
            />
          );
        })()}
    </div>
  );
}

/**
 * A right-click menu (task 8.7): fixed at the click, kept inside the window, one level of
 * submenus or more. Arrows move, Enter runs, right opens a submenu, left and Esc go back, a
 * click outside closes it. The focus goes back where it was when it closes.
 */
export function ContextMenu({ x, y, items, label, onClose }: ContextMenuProps) {
  const before = useRef<Element | null>(null);
  useLayoutEffect(() => {
    before.current = document.activeElement;
    return () => {
      const el = before.current;
      if (el instanceof HTMLElement && el.isConnected) el.focus({ preventScroll: true });
    };
  }, []);
  return (
    <div
      className="ctx-back"
      onPointerDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <Panel x={x} y={y} items={items} label={label} depth={0} onClose={onClose} />
    </div>
  );
}

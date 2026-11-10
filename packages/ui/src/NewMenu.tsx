import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useChord } from "./chords";

/** A plugin's entry after Terminal and Text, such as "New row from worktree…". */
export interface NewMenuItem {
  id: string;
  label: string;
  hint?: string;
  run(): void;
}

/** `+ ▾`: a terminal or a text. The tab bar makes tabs; the sidebar makes side tabs. */
export function NewMenu({
  onNew,
  onNewText,
  hints,
  textTitle = "Hold ⌥ to split the focused pane",
  className = "ao-tab-new more",
  label = "Choose tab kind",
  extra = [],
}: {
  onNew(): void;
  onNewText(beside: boolean): void;
  hints?: readonly [string, string?];
  textTitle?: string;
  className?: string;
  label?: string;
  extra?: readonly NewMenuItem[];
}) {
  const newTab = useChord("new_tab");
  const newText = useChord("new_text");
  const [termHint, textHint] = hints ?? [newTab, newText];
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLSpanElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  // A menu that opens upward, as the sidebar's does, opens downward when the scrolling list
  // above would cut its top off: plugin entries make it taller than the space above one row.
  const [down, setDown] = useState(false);
  useLayoutEffect(() => {
    if (!open) return setDown(false);
    const m = menu.current;
    if (!m || down) return;
    let clip: HTMLElement | null = m.parentElement;
    while (clip && getComputedStyle(clip).overflowY === "visible") clip = clip.parentElement;
    const top = clip ? clip.getBoundingClientRect().top : 0;
    if (m.getBoundingClientRect().top < top) setDown(true);
  }, [open, down, extra.length]);
  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => !box.current?.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("pointerdown", away);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("pointerdown", away);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);
  return (
    <span ref={box} className="tab-new-menu">
      <button type="button" className={className} aria-label={label} aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        ▾
      </button>
      {open && (
        <div ref={menu} role="menu" className="tab-menu ao-frame" data-down={down || undefined}>
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setOpen(false);
              onNew();
            }}
          >
            <span className="grow">Terminal</span>
            {termHint && <span className="small ao-dim">{termHint}</span>}
          </button>
          <button
            type="button"
            role="menuitem"
            title={textTitle}
            onClick={(e) => {
              setOpen(false);
              onNewText(e.altKey);
            }}
          >
            <span className="grow">Text</span>
            {textHint && <span className="small ao-dim">{textHint}</span>}
          </button>
          {extra.map((x) => (
            <button
              key={x.id}
              type="button"
              role="menuitem"
              data-action={x.id}
              onClick={() => {
                setOpen(false);
                x.run();
              }}
            >
              <span className="grow">{x.label}</span>
              {x.hint && <span className="small ao-dim">{x.hint}</span>}
            </button>
          ))}
        </div>
      )}
    </span>
  );
}

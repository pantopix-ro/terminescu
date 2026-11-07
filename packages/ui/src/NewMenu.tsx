import { useEffect, useRef, useState } from "react";

/** `+ ▾`: a terminal or a text. The tab bar makes tabs; the sidebar makes side tabs. */
export function NewMenu({
  onNew,
  onNewText,
  hints = ["⌘T", "⌥⌘T"],
  textTitle = "Hold ⌥ to split the focused pane",
  className = "ao-tab-new more",
  label = "Choose tab kind",
}: {
  onNew(): void;
  onNewText(beside: boolean): void;
  hints?: readonly [string, string?];
  textTitle?: string;
  className?: string;
  label?: string;
}) {
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLSpanElement>(null);
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
        <div role="menu" className="tab-menu ao-frame">
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setOpen(false);
              onNew();
            }}
          >
            <span className="grow">Terminal</span>
            <span className="small ao-dim">{hints[0]}</span>
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
            {hints[1] && <span className="small ao-dim">{hints[1]}</span>}
          </button>
        </div>
      )}
    </span>
  );
}

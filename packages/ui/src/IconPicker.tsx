import { ICONS, type Icon } from "@terminescu/host";
import { useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";

/** What each icon of {@link ICONS} shows, for its tooltip and for screen readers. */
export const ICON_LABELS: Record<Icon, string> = {
  term: "Terminal",
  agent: "Agent",
  group: "Group",
  tab: "Tab",
  window: "Window",
  split: "Split",
  doc: "Document",
  tpl: "Template",
  prompt: "Prompt",
  var: "Variable",
  link: "Link",
  pin: "Pin",
  play: "Play",
  check: "Check",
  bp: "Dot",
  pr: "Branch",
  merge: "Merge",
  save: "Save",
  in: "Download",
  out: "Upload",
};

const COLUMNS = 5;
const GAP = 4;

export interface IconPickerProps {
  /** Where the double click landed, in window pixels. */
  x: number;
  y: number;
  /** The name of the side tab or tab, for the dialog label. */
  name: string;
  current?: Icon;
  /** `null` goes back to the default icon. */
  onPick(icon: Icon | null): void;
  onClose(): void;
}

/**
 * The icon list of a side tab or tab (task 8.16): a grid at the double click, kept inside
 * the window. Arrows move, Enter picks, Esc or a click outside closes; the focus goes back
 * where it was.
 */
export function IconPicker({ x, y, name, current, onPick, onClose }: IconPickerProps) {
  const ref = useRef<HTMLDivElement>(null);
  const before = useRef<Element | null>(null);
  const [pos, setPos] = useState<{ left: number; top: number }>();
  useLayoutEffect(() => {
    before.current = document.activeElement;
    const el = ref.current!;
    const left = Math.max(GAP, Math.min(x, window.innerWidth - GAP - el.offsetWidth));
    const top = Math.max(GAP, Math.min(y, window.innerHeight - GAP - el.offsetHeight));
    setPos({ left, top });
    (el.querySelector<HTMLElement>("[aria-pressed=true]") ?? el.querySelector<HTMLElement>("button"))?.focus();
    return () => {
      const back = before.current;
      if (back instanceof HTMLElement && back.isConnected) back.focus({ preventScroll: true });
    };
  }, [x, y]);
  // A modal list: a pane that grabs the focus as its tab shows hands it straight back.
  useLayoutEffect(() => {
    const keep = (e: FocusEvent) => {
      const el = ref.current;
      if (el && !el.contains(e.target as Node)) (el.querySelector<HTMLElement>("[aria-pressed=true]") ?? el.querySelector<HTMLElement>("button"))?.focus();
    };
    document.addEventListener("focusin", keep);
    return () => document.removeEventListener("focusin", keep);
  }, []);

  const pick = (icon: Icon | null) => {
    onClose();
    onPick(icon);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    // The picker owns the keyboard while it is open; nothing reaches the pane under it.
    e.stopPropagation();
    const all = [...(ref.current?.querySelectorAll<HTMLElement>(".icon-cell") ?? [])];
    const at = all.indexOf(document.activeElement as HTMLElement);
    const by = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: COLUMNS, ArrowUp: -COLUMNS }[e.key];
    if (by !== undefined) all[at < 0 ? 0 : Math.min(all.length - 1, Math.max(0, at + by))]?.focus();
    else if (e.key === "Escape") onClose();
    else if (e.key === "Tab") return;
    else return;
    e.preventDefault();
  };

  return (
    <div
      className="ctx-back"
      onPointerDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
      onContextMenu={(e) => e.preventDefault()}
    >
      <div
        ref={ref}
        className="ao-frame icon-picker"
        role="dialog"
        aria-label={`Icon for ${name}`}
        style={pos ?? { left: x, top: y, visibility: "hidden" }}
        onKeyDown={onKeyDown}
      >
        <div className="icon-grid">
          {ICONS.map((icon) => (
            <button
              key={icon}
              type="button"
              className="icon-cell"
              aria-label={ICON_LABELS[icon]}
              aria-pressed={icon === current}
              title={ICON_LABELS[icon]}
              onClick={() => pick(icon)}
            >
              <i className={`ao-i ao-i-${icon}`} aria-hidden="true" />
            </button>
          ))}
        </div>
        <button type="button" className="ao-btn icon-default" disabled={!current} onClick={() => pick(null)}>
          Default icon
        </button>
      </div>
    </div>
  );
}

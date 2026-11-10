import { MAX_LABEL, type ManualStatus, type SavedStatus } from "@terminescu/host";
import { useState, type KeyboardEvent } from "react";
import { usePopover } from "./popover";
import { StatusChip } from "./StatusChip";
import type { Chip } from "./status";
import { useChord } from "./chords";

export interface QuickStatusProps {
  /** Under the clicked chip, in window pixels. */
  x: number;
  y: number;
  /** The side tab or tab, for the dialog label. */
  name: string;
  /** Its manual status, absent when the generated one shows. */
  current?: ManualStatus;
  /** What the generated status says now. */
  auto: Chip;
  /** This side tab's saved statuses, then the global ones. */
  local: SavedStatus[];
  global: SavedStatus[];
  /** Sets a status, or `null` for back to auto. */
  onSet(status: ManualStatus | null): void;
  /** Opens the full status editor. */
  onEdit(): void;
  onClose(): void;
}

const chipOf = (s: { label: string; accent: ManualStatus["accent"]; background: ManualStatus["background"] }): Chip => ({ ...s, set: true, need: false });
const lookOf = (s: SavedStatus): ManualStatus => ({ statusId: s.id, label: s.label, accent: s.accent, background: s.background });

/**
 * A click on a sidebar status chip (task 8.21): type a new status, or pick the current one,
 * auto, this side tab's or a global status. The box takes the keyboard; Enter sets what it
 * holds, the arrows walk the list, Esc closes.
 */
export function QuickStatus(props: QuickStatusProps) {
  const editChord = useChord("status_editor");
  const { current, local, global } = props;
  const { ref, style } = usePopover(props.x, props.y, [".qs-input"]);
  const [text, setText] = useState("");
  const set = (s: ManualStatus | null) => {
    props.onClose();
    props.onSet(s);
  };
  // A status set by hand that is not one of the saved ones stays offered as itself.
  const own = current && !current.statusId && [...local, ...global].every((s) => s.label !== current.label) ? current : undefined;
  const picked = (s: SavedStatus) => current?.statusId === s.id || (!current?.statusId && current?.label === s.label);

  const onKeyDown = (e: KeyboardEvent) => {
    // The list owns the keyboard while it is open; nothing reaches the pane under it.
    e.stopPropagation();
    const all = [...(ref.current?.querySelectorAll<HTMLElement>(".qs-input, .qs-item") ?? [])];
    const at = all.indexOf(document.activeElement as HTMLElement);
    if (e.key === "ArrowDown") all[Math.min(all.length - 1, at + 1)]?.focus();
    else if (e.key === "ArrowUp") all[Math.max(0, at - 1)]?.focus();
    else if (e.key === "Escape") props.onClose();
    else return;
    e.preventDefault();
  };
  const item = (key: string, label: string, chip: Chip, checked: boolean, run: () => void) => (
    <button key={key} type="button" role="menuitemradio" aria-checked={checked} aria-label={label} className="qs-item" onClick={run}>
      <StatusChip chip={chip} />
      <span className="grow" />
      {checked && <i className="ao-i x1 ao-i-check" aria-hidden="true" />}
    </button>
  );

  return (
    <div
      className="ctx-back"
      onPointerDown={(e) => {
        if (e.target === e.currentTarget) props.onClose();
      }}
      onContextMenu={(e) => e.preventDefault()}
    >
      <div ref={ref} className="ao-frame quick-status" role="dialog" aria-label={`Status of ${props.name}`} style={style} onKeyDown={onKeyDown}>
        <input
          className="ao-input qs-input"
          aria-label="New status"
          placeholder="New status, ↵ to set"
          value={text}
          maxLength={MAX_LABEL}
          spellCheck={false}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key !== "Enter" || !text.trim()) return;
            e.preventDefault();
            set({ label: text.trim(), accent: current?.accent ?? null, background: current?.background ?? null });
          }}
        />
        <div role="menu" aria-label="Statuses" className="qs-list">
          {own && item("own", own.label, chipOf(own), true, () => set(own))}
          {item("auto", "Auto", props.auto, !current, () => set(null))}
          {local.length > 0 && <div className="qs-head ao-pixel ao-dim">This side tab</div>}
          {local.map((s) => item(s.id, s.label, chipOf(s), picked(s), () => set(lookOf(s))))}
          <div className="qs-head ao-pixel ao-dim">Global</div>
          {!global.length && <div className="small ao-dim">No global statuses yet.</div>}
          {global.map((s) => item(s.id, s.label, chipOf(s), picked(s), () => set(lookOf(s))))}
        </div>
        <button type="button" className="qs-item qs-edit" onClick={() => (props.onClose(), props.onEdit())}>
          <span className="grow">Edit status…</span>
          {editChord && <span className="ctx-hint">{editChord}</span>}
        </button>
      </div>
    </div>
  );
}

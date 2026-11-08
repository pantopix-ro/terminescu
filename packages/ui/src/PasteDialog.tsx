import { useEffect, useRef } from "react";

export interface PasteDialogProps {
  text: string;
  /** Why it asks, from `pasteRisk`. */
  reason: string;
  onPaste(): void;
  onCancel(): void;
}

/** The first lines of a paste, so the person sees what would run. */
const preview = (text: string) => {
  const lines = text.split(/\r\n|\r|\n/);
  const head = lines.slice(0, 8).map((l) => (l.length > 160 ? `${l.slice(0, 160)}…` : l));
  return lines.length > 8 ? [...head, `… ${lines.length - 8} more lines`].join("\n") : head.join("\n");
};

/** Asks before a risky paste reaches the shell (task 8.42); Return pastes, Esc cancels. */
export function PasteDialog({ text, reason, onPaste, onCancel }: PasteDialogProps) {
  const paste = useRef<HTMLButtonElement>(null);
  useEffect(() => paste.current?.focus(), []);
  return (
    <div className="sheet-back" onPointerDown={onCancel}>
      <div
        className="ao-frame paste-dialog"
        role="alertdialog"
        aria-modal="true"
        aria-label="Paste into the terminal?"
        onPointerDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            onCancel();
          }
        }}
      >
        <div className="ao-frame-body">
          <p>
            <b>Paste into the terminal?</b> {reason}
          </p>
          <pre className="paste-preview">{preview(text)}</pre>
          <div className="srow">
            <span className="small ao-dim grow">Settings › Terminal turns this check off</span>
            <button type="button" className="ao-btn ghost" onClick={onCancel}>
              Cancel
            </button>
            <button ref={paste} type="button" className="ao-btn primary" onClick={onPaste}>
              Paste
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

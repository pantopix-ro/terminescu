import { VAR_MODES, type NewVariable, type VarMode } from "@terminescu/host";
import { useState } from "react";
import { parseTtl, suggestName } from "./names";

export interface ExtractDialogProps {
  /** The selected text. */
  value: string;
  /** Names already used in the side tab, so the suggestion is free. */
  taken: string[];
  /** Where the text came from, for the hint under the name. */
  from: string;
  save(variable: Pick<NewVariable, "name" | "mode" | "ttlSecs">): Promise<unknown>;
  onClose(): void;
}

const MODE_LABEL: Record<VarMode, string> = { saved: "Saved", session: "Session only", secret: "Secret" };
const MODE_HINT: Record<VarMode, string> = {
  saved: "kept in the database",
  session: "kept in memory until a reboot or the side tab closes",
  secret: "in memory, masked, inserted as $NAME",
};

/**
 * `⌘E`: saves a selection as a variable of the side tab (D13, D21). The name is suggested and
 * editable; the mode decides whether the value ever touches the disk.
 */
export function ExtractDialog({ value, taken, from, save, onClose }: ExtractDialogProps) {
  const [name, setName] = useState(() => suggestName(value, taken));
  const [mode, setMode] = useState<VarMode>("saved");
  const [ttl, setTtl] = useState("");
  const [error, setError] = useState<string>();
  const submit = async () => {
    const ttlSecs = mode === "saved" || !ttl.trim() ? null : parseTtl(ttl);
    if (ttlSecs === undefined) return setError("TTL reads like 30m, 1h or 2d");
    try {
      await save({ name: name.trim(), mode, ttlSecs });
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <div className="sheet-back" onPointerDown={onClose}>
    <form
      className="ao-frame x-dialog"
      role="dialog"
      aria-label="Extract to variable"
      onPointerDown={(e) => e.stopPropagation()}
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          onClose();
        }
      }}
    >
      <div className="ao-frame-head">
        <i className="ao-i ao-i-var" />
        <span className="ao-pixel">Extract</span>
        <span className="grow" />
        <span className="small ao-dim">{from}</span>
      </div>
      <div className="ao-frame-body x-body">
      <label className="x-row">
        <span className="small ao-dim">Name</span>
        <span className="x-name">
          <span className="ao-dim">{"{{"}</span>
          <input
            className="ao-input"
            aria-label="Variable name"
            autoFocus
            spellCheck={false}
            value={name}
            onChange={(e) => {
              setName(e.target.value);
              setError(undefined);
            }}
          />
          <span className="ao-dim">{"}}"}</span>
        </span>
      </label>
      <div className="x-row">
        <span className="small ao-dim">Value</span>
        <span className={`x-value small${mode === "secret" ? " masked" : ""}`} title={mode === "secret" ? undefined : value}>
          {mode === "secret" ? "•".repeat(Math.min(12, Math.max(4, value.length))) : value}
        </span>
      </div>
      <div className="x-row" role="radiogroup" aria-label="Storage">
        <span className="small ao-dim">Keep</span>
        <span className="x-modes">
          {VAR_MODES.map((m) => (
            <button
              key={m}
              type="button"
              role="radio"
              aria-checked={m === mode}
              className={`ao-btn ${m === mode ? "primary" : "ghost"} x-mode`}
              onClick={() => setMode(m)}
            >
              {MODE_LABEL[m]}
            </button>
          ))}
        </span>
      </div>
      <p className="small ao-dim x-hint">{MODE_HINT[mode]}</p>
      {mode !== "saved" && (
        <label className="x-row">
          <span className="small ao-dim">TTL</span>
          <input
            className="ao-input x-ttl"
            aria-label="Time to live"
            placeholder="none, or 1h"
            value={ttl}
            onChange={(e) => setTtl(e.target.value)}
          />
        </label>
      )}
      {error && (
        <p className="small app-error" role="alert">
          {error}
        </p>
      )}
      <div className="x-foot">
        <button type="button" className="ao-btn ghost" onClick={onClose}>
          Cancel
        </button>
        <button type="submit" className="ao-btn primary">
          Save ↵
        </button>
      </div>
      </div>
    </form>
    </div>
  );
}

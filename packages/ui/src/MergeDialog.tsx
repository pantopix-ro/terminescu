import type { MergeClash, MergeResolution } from "@terminescu/host";
import { isVariableName } from "@terminescu/host";
import { useState } from "react";

export interface MergeDialogProps {
  fromName: string;
  intoName: string;
  clashes: MergeClash[];
  onMerge(resolution: MergeResolution): void;
  onCancel(): void;
}

const shown = (value: string | null) => value ?? "••••••";

/**
 * Asks how to merge two side tabs whose variables clash (D5): keep the target's value, or keep
 * both with the source's renamed. A secret shows masked; its value never reaches the window.
 */
export function MergeDialog({ fromName, intoName, clashes, onMerge, onCancel }: MergeDialogProps) {
  const [names, setNames] = useState<Record<string, string>>(() => Object.fromEntries(clashes.map((c) => [c.name, c.rename])));
  const typed = Object.values(names);
  const bad = clashes.find((c) => {
    const n = names[c.name] ?? "";
    return !isVariableName(n) || typed.filter((x) => x === n).length > 1;
  });
  return (
    <div className="sheet-back" onPointerDown={onCancel}>
      <div
        className="ao-frame merge-dialog"
        role="dialog"
        aria-label="Merge side tabs"
        onPointerDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.key === "Escape" && onCancel()}
      >
        <div className="ao-frame-body">
          <p>
            Merge <b>{fromName}</b> into <b>{intoName}</b>.{" "}
            {clashes.length === 1 ? "One variable has" : `${clashes.length} variables have`} two values.
          </p>
          <table className="merge-clashes">
            <thead>
              <tr>
                <th>Variable</th>
                <th>{intoName}</th>
                <th>{fromName}</th>
                <th>Kept as</th>
              </tr>
            </thead>
            <tbody>
              {clashes.map((c, i) => (
                <tr key={c.name} aria-label={`Clash ${c.name}`}>
                  <td className="mono">{c.name}</td>
                  <td className="mono">{shown(c.intoValue)}</td>
                  <td className="mono">{shown(c.fromValue)}</td>
                  <td>
                    <input
                      className="ao-input mono"
                      aria-label={`New name for ${fromName}'s ${c.name}`}
                      value={names[c.name] ?? ""}
                      spellCheck={false}
                      autoFocus={i === 0}
                      onChange={(e) => {
                        const v = e.target.value.trim();
                        setNames((m) => ({ ...m, [c.name]: v }));
                      }}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {bad && <p className="small app-error-inline" role="alert">{names[bad.name] || "A name"} cannot hold {fromName}'s {bad.name}.</p>}
          <div className="close-actions">
            <button type="button" className="ao-btn ghost" onClick={onCancel}>
              Cancel
            </button>
            <span className="grow" />
            <button type="button" className="ao-btn ghost" onClick={() => onMerge({ keepBoth: false })}>
              Keep {intoName}'s
            </button>
            <button type="button" className="ao-btn primary" disabled={!!bad} onClick={() => onMerge({ keepBoth: true, renames: names })}>
              Keep both
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

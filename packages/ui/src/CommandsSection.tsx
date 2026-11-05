import type { CommandTarget, RunOutcome, SavedCommand } from "@terminescu/host";
import { useState } from "react";

export const TARGET_LABEL: Record<CommandTarget, string> = {
  focused: "focused pane",
  newTab: "new tab",
  newSideTab: "new side tab",
};

export interface CommandDraft {
  name: string;
  body: string;
  target: CommandTarget;
  global: boolean;
}

export interface CommandsSectionProps {
  /** The side tab's own commands, then the global ones. */
  commands: SavedCommand[];
  /** No side tab, no commands to run or add. */
  enabled: boolean;
  onAdd(draft: CommandDraft): Promise<unknown>;
  onDelete(id: string): Promise<unknown>;
  /** Run sends Enter, insert does not; `extra` holds values typed for missing names. */
  onRun(command: SavedCommand, run: boolean, extra: Record<string, string>): Promise<RunOutcome>;
}

interface Asking {
  command: SavedCommand;
  run: boolean;
  names: string[];
  values: Record<string, string>;
}

/**
 * The Commands section of the sidebar (D24): saved shell lines with `{{variables}}`. Run types
 * the line and Enter into the command's target, Insert types it without Enter. A missing name
 * is asked for here and never saved.
 */
export function CommandsSection({ commands, enabled, onAdd, onDelete, onRun }: CommandsSectionProps) {
  const [adding, setAdding] = useState(false);
  const [draft, setDraft] = useState<CommandDraft>({ name: "", body: "", target: "newTab", global: false });
  const [asking, setAsking] = useState<Asking>();
  const [error, setError] = useState<string>();

  const go = async (command: SavedCommand, run: boolean, extra: Record<string, string> = {}) => {
    setError(undefined);
    try {
      const out = await onRun(command, run, extra);
      if (out.kind === "missing") return setAsking({ command, run, names: out.names, values: extra });
      setAsking(undefined);
      if (out.kind === "unsafe")
        setError(`${out.names.join(", ")} holds a line break: Insert it and read it before pressing Enter.`);
    } catch (e) {
      setError(String(e));
    }
  };

  const save = async () => {
    setError(undefined);
    try {
      await onAdd(draft);
      setDraft({ name: "", body: "", target: draft.target, global: false });
      setAdding(false);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <section className="cmd-section" aria-label="Commands">
      <header className="cmd-head">
        <span className="ao-pixel small grow">Commands</span>
        <button type="button" className="ao-btn ghost cmd-new" aria-label="New command" disabled={!enabled} onClick={() => setAdding((a) => !a)}>
          +
        </button>
      </header>
      {adding && (
        <form
          className="cmd-form"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <input className="ao-input" aria-label="Command name" placeholder="name" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
          <textarea
            className="ao-input cmd-body-input"
            aria-label="Command"
            placeholder="gh pr checks {{pr_api}}"
            rows={2}
            value={draft.body}
            onChange={(e) => setDraft({ ...draft, body: e.target.value })}
          />
          <div className="cmd-form-row">
            <select className="ao-input" aria-label="Run in" value={draft.target} onChange={(e) => setDraft({ ...draft, target: e.target.value as CommandTarget })}>
              {(Object.keys(TARGET_LABEL) as CommandTarget[]).map((t) => (
                <option key={t} value={t}>
                  {TARGET_LABEL[t]}
                </option>
              ))}
            </select>
            <label className="small">
              <input type="checkbox" checked={draft.global} onChange={(e) => setDraft({ ...draft, global: e.target.checked })} /> global
            </label>
            <span className="grow" />
            <button type="submit" className="ao-btn">
              Save
            </button>
          </div>
        </form>
      )}
      <div className="cmd-list">
        {!commands.length && !adding && <p className="small ao-dim cmd-empty">No commands yet.</p>}
        {commands.map((c) => (
          <div key={c.id} className="cmd-row" data-command={c.name}>
            <span className="cmd-name">{c.name}</span>
            <span className="cmd-body" title={c.body}>
              {c.body}
            </span>
            <span className="cmd-meta small ao-dim">
              {TARGET_LABEL[c.target]}
              {c.sideTabId === null ? " · global" : ""}
            </span>
            <span className="cmd-acts">
              <button type="button" className="ao-btn" aria-label={`Run ${c.name}`} disabled={!enabled} onClick={() => void go(c, true)}>
                Run
              </button>
              <button type="button" className="ao-btn ghost" aria-label={`Insert ${c.name}`} disabled={!enabled} onClick={() => void go(c, false)}>
                Insert
              </button>
              <button type="button" className="ao-btn ghost" aria-label={`Delete ${c.name}`} onClick={() => void onDelete(c.id).catch((e: unknown) => setError(String(e)))}>
                ×
              </button>
            </span>
          </div>
        ))}
      </div>
      {asking && (
        <form
          className="cmd-ask"
          onSubmit={(e) => {
            e.preventDefault();
            void go(asking.command, asking.run, asking.values);
          }}
        >
          <p className="small">Values for {asking.command.name}, used this once:</p>
          {asking.names.map((n, i) => (
            <input
              key={n}
              className="ao-input"
              aria-label={n}
              placeholder={n}
              autoFocus={i === 0}
              value={asking.values[n] ?? ""}
              onChange={(e) => setAsking({ ...asking, values: { ...asking.values, [n]: e.target.value } })}
              onKeyDown={(e) => e.key === "Escape" && setAsking(undefined)}
            />
          ))}
          <div className="cmd-form-row">
            <span className="grow" />
            <button type="button" className="ao-btn ghost" onClick={() => setAsking(undefined)}>
              Cancel
            </button>
            <button type="submit" className="ao-btn">
              {asking.run ? "Run" : "Insert"}
            </button>
          </div>
        </form>
      )}
      {error && (
        <p className="small app-error-inline" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}

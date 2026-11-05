import type { CommandTarget, Id, RunOutcome, SavedCommand, Shown } from "@terminescu/host";
import { useRef, useState } from "react";

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
  /** Runs in its own tab when the side tab opens and after a reboot (D24). */
  launcher: boolean;
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
  /** Shown statuses: a launcher command is up while its tab runs something. */
  statuses: Record<Id, Shown>;
  /** Runs launcher commands that are down again: one, or all of the side tab's. */
  onRestart(id?: Id): Promise<unknown>;
}

/** Whether a launcher command runs: its tab's auto status is Running. */
export const launcherUp = (c: SavedCommand, statuses: Record<Id, Shown>) =>
  c.tabId !== null && statuses[c.tabId]?.auto === "running";

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
export function CommandsSection({ commands, enabled, onAdd, onDelete, onRun, statuses, onRestart }: CommandsSectionProps) {
  const [adding, setAdding] = useState(false);
  const [draft, setDraft] = useState<CommandDraft>({ name: "", body: "", target: "newTab", global: true, launcher: false });
  const wasGlobal = useRef(true);
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
      setDraft({ name: "", body: "", target: draft.target, global: true, launcher: false });
      setAdding(false);
    } catch (e) {
      setError(String(e));
    }
  };

  const restart = (id?: Id) => {
    setError(undefined);
    void onRestart(id).catch((e: unknown) => setError(String(e)));
  };
  const down = commands.filter((c) => c.launcher && !launcherUp(c, statuses));

  return (
    <section className="cmd-section" aria-label="Commands">
      <header className="cmd-head">
        <span className="ao-pixel small grow">Commands</span>
        {down.length > 0 && (
          <button type="button" className="ao-btn ghost" disabled={!enabled} onClick={() => restart()}>
            Restart down
          </button>
        )}
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
              <input
                type="checkbox"
                disabled={draft.launcher}
                checked={!draft.global}
                onChange={(e) => setDraft({ ...draft, global: !e.target.checked })}
              />{" "}
              this side tab only
            </label>
            <label className="small" title="Runs in its own tab when the side tab opens and after a reboot; stays in this side tab">
              <input
                type="checkbox"
                checked={draft.launcher}
                onChange={(e) => {
                  // A launcher stays in this side tab; unticking it gives back the scope it had.
                  if (e.target.checked) wasGlobal.current = draft.global;
                  setDraft({ ...draft, launcher: e.target.checked, global: e.target.checked ? false : wasGlobal.current });
                }}
              />{" "}
              launcher
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
              {c.launcher && (
                <span className={launcherUp(c, statuses) ? "cmd-up" : "cmd-down"}>{launcherUp(c, statuses) ? " · launcher · up" : " · launcher · down"}</span>
              )}
            </span>
            <span className="cmd-acts">
              {c.launcher && !launcherUp(c, statuses) && (
                <button type="button" className="ao-btn" aria-label={`Restart ${c.name}`} disabled={!enabled} onClick={() => restart(c.id)}>
                  Restart
                </button>
              )}
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

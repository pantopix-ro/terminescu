import { MAX_LABEL, type Auto, type Host, type Look, type Rule, type RuleKind, type SavedStatus, type StatusConfig } from "@terminescu/host";
import { useEffect, useState } from "react";
import { StatusChip } from "./StatusChip";
import { BUILTIN_LOOKS, builtinLook } from "./status";
import { Swatches } from "./Swatches";

type Api = Pick<
  Host,
  "saveStatus" | "updateStatus" | "deleteStatus" | "reorderStatuses" | "addRule" | "toggleRule" | "deleteRule" | "setBuiltin"
>;

export interface StatusesSectionProps {
  config: StatusConfig;
  host: Api;
  onError(message: string): void;
}

const ORDER: Auto[] = ["waiting", "running", "done", "failed", "idle"];
const MEANS: Record<Auto, string> = {
  waiting: "Claude asks for permission or a question",
  running: "command running, or Claude working",
  done: "Claude finished its turn, last command exit 0",
  failed: "last command exited non-zero",
  idle: "shell at the prompt, nothing new",
};

/** A label field that saves on Enter or when it loses focus, and only when it changed. */
function LabelField({ value, label, onSave }: { value: string; label: string; onSave(v: string): void }) {
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);
  const commit = () => {
    const t = text.trim();
    if (t && t !== value) onSave(t);
    else setText(value);
  };
  return (
    <input
      className="ao-input"
      value={text}
      maxLength={MAX_LABEL}
      aria-label={label}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      // Escape closes the sheet, which unmounts the field without a blur: commit first.
      onKeyDown={(e) => (e.key === "Enter" || e.key === "Escape") && commit()}
    />
  );
}

function ruleText(r: Rule) {
  if (r.kind === "exitCode")
    return (
      <>
        When a command matching <code>{r.pattern}</code> exits non-zero
      </>
    );
  if (r.kind === "fact")
    return (
      <>
        While <code>{r.name}</code> holds
      </>
    );
  return <>When a Claude session is idle for {r.minutes} min</>;
}

const UNTIL: Record<Rule["kind"], string> = {
  exitCode: "until the next run",
  claudeIdle: "until activity",
  fact: "while it holds",
};

/** Settings › Statuses: built-ins, the global list and the rules (design board "Settings"). */
export function StatusesSection({ config, host, onError }: StatusesSectionProps) {
  const run = (p: Promise<unknown>) => void p.catch((e: unknown) => onError(String(e)));
  const global = config.statuses.filter((s) => s.sideTabId === null);
  const [newLabel, setNewLabel] = useState("");
  const [ruleKind, setRuleKind] = useState<RuleKind["kind"]>("exitCode");
  const [ruleArg, setRuleArg] = useState("");
  const [ruleStatus, setRuleStatus] = useState("");

  const move = (s: SavedStatus, by: number) => {
    const ids = global.map((g) => g.id);
    const at = ids.indexOf(s.id);
    const to = at + by;
    if (to < 0 || to >= ids.length) return;
    [ids[at], ids[to]] = [ids[to]!, ids[at]!];
    run(host.reorderStatuses(ids));
  };
  const update = (s: SavedStatus, patch: Partial<Look>) =>
    run(host.updateStatus(s.id, { label: s.label, accent: s.accent, background: s.background, ...patch }));
  const addRule = () => {
    const status = ruleStatus || config.statuses[0]?.id;
    if (!status) return onError("Save a status first; a rule sets one.");
    const kind: RuleKind = ruleKind === "exitCode" ? { kind: "exitCode", pattern: ruleArg } : { kind: "claudeIdle", minutes: Number(ruleArg) };
    run(host.addRule(status, kind).then(() => setRuleArg("")));
  };

  return (
    <>
          <section aria-label="Built-in">
            <h3 className="ao-pixel ao-dim">Built-in</h3>
            {ORDER.map((auto) => {
              const look = builtinLook(auto, config.builtins);
              return (
                <div key={auto} className="srow builtin" data-builtin={auto}>
                  <StatusChip chip={{ ...look, set: false, need: auto === "waiting" }} />
                  <LabelField value={look.label} label={`${BUILTIN_LOOKS[auto].label} label`} onSave={(label) => run(host.setBuiltin(auto, { ...look, label }))} />
                  {auto === "waiting" ? (
                    <span className="small ao-dim">amber</span>
                  ) : (
                    <Swatches kind="accent" small value={look.accent} onChange={(accent) => run(host.setBuiltin(auto, { ...look, accent }))} />
                  )}
                  <span className="small ao-dim grow">{MEANS[auto]}</span>
                  {config.builtins[auto] && (
                    <button type="button" className="link small" onClick={() => run(host.setBuiltin(auto, null))}>
                      reset
                    </button>
                  )}
                </div>
              );
            })}
          </section>
          <section aria-label="Global">
            <h3 className="ao-pixel ao-dim">Global</h3>
            {global.length === 0 && <p className="small ao-dim">No global statuses yet. Add one here or promote one from the editor.</p>}
            {global.map((s, i) => (
              <div key={s.id} className="srow" data-global={s.label}>
                <StatusChip chip={{ ...s, set: true, need: false }} />
                <LabelField value={s.label} label={`${s.label} label`} onSave={(label) => update(s, { label })} />
                <Swatches kind="accent" small value={s.accent} onChange={(accent) => update(s, { accent })} />
                <Swatches kind="background" small value={s.background} onChange={(background) => update(s, { background })} />
                <span className="grow" />
                <button type="button" className="ao-btn ghost" aria-label={`Move ${s.label} up`} disabled={i === 0} onClick={() => move(s, -1)}>
                  ↑
                </button>
                <button type="button" className="ao-btn ghost" aria-label={`Move ${s.label} down`} disabled={i === global.length - 1} onClick={() => move(s, 1)}>
                  ↓
                </button>
                <button type="button" className="ao-btn ghost" aria-label={`Delete ${s.label}`} onClick={() => run(host.deleteStatus(s.id))}>
                  ×
                </button>
              </div>
            ))}
            <form
              className="srow"
              onSubmit={(e) => {
                e.preventDefault();
                if (newLabel.trim()) run(host.saveStatus(null, { label: newLabel, accent: null, background: null }).then(() => setNewLabel("")));
              }}
            >
              <input className="ao-input" value={newLabel} maxLength={MAX_LABEL} placeholder="New status" aria-label="New global status" onChange={(e) => setNewLabel(e.target.value)} />
              <button type="submit" className="ao-btn" disabled={!newLabel.trim()}>
                Add
              </button>
            </form>
            <p className="small ao-dim">Deleting a status sends the tabs that use it back to auto.</p>
          </section>
          <section aria-label="Rules">
            <h3 className="ao-pixel ao-dim">Rules</h3>
            {config.rules.map((r) => (
              <div key={r.id} className="srow" data-rule={r.id}>
                <span className="grow">{ruleText(r)}</span>
                <StatusChip chip={{ ...r.look, set: true, need: false }} />
                <span className="small ao-dim">{UNTIL[r.kind]}</span>
                <label className="small">
                  <input type="checkbox" checked={r.enabled} onChange={(e) => run(host.toggleRule(r.id, e.target.checked))} /> on
                </label>
                <button type="button" className="ao-btn ghost" aria-label="Delete rule" onClick={() => run(host.deleteRule(r.id))}>
                  ×
                </button>
              </div>
            ))}
            <div className="srow">
              <span className="grow">
                When <code>terminescu status</code> is called from a shell
              </span>
              <span>its label</span>
              <span className="small ao-dim">until exit or --for</span>
            </div>
            <form
              className="srow"
              onSubmit={(e) => {
                e.preventDefault();
                addRule();
              }}
            >
              <select className="ao-input" value={ruleKind} aria-label="Rule kind" onChange={(e) => setRuleKind(e.target.value as RuleKind["kind"])}>
                <option value="exitCode">command exits non-zero</option>
                <option value="claudeIdle">Claude idle (minutes)</option>
              </select>
              <input
                className="ao-input"
                value={ruleArg}
                inputMode={ruleKind === "claudeIdle" ? "numeric" : undefined}
                placeholder={ruleKind === "exitCode" ? "pnpm test*" : "30"}
                aria-label={ruleKind === "exitCode" ? "Command pattern" : "Minutes"}
                onChange={(e) => setRuleArg(e.target.value)}
              />
              <select className="ao-input" value={ruleStatus} aria-label="Status to set" onChange={(e) => setRuleStatus(e.target.value)}>
                {config.statuses.length === 0 && <option value="">no saved status</option>}
                {config.statuses.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.label}
                    {s.sideTabId ? " (local)" : ""}
                  </option>
                ))}
              </select>
              <button type="submit" className="ao-btn" disabled={!ruleArg.trim() || config.statuses.length === 0}>
                Add rule
              </button>
            </form>
          </section>
    </>
  );
}

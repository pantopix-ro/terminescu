// Settings › Claude (add-claude-insights 3.4): the meter threshold, a context window per model,
// and the baselines' retention, total size and largest file. The values live in the Rust half's plugin settings; it
// checks each one and answers the settings as they now are.

import { useEffect, useState } from "react";
import type { SettingsPageProps } from "@terminescu/plugin-api";

/** The Rust half's `settings` answer. */
export interface ClaudeSettings {
  meterThreshold: number;
  contextWindows: Record<string, number>;
  baselineCapMb: number;
  baselineDays: number;
  /** The largest file a baseline keeps (polish-claude-insights D3). */
  baselineFileMb: number;
  defaults: { meterThreshold: number; window: number; baselineCapMb: number; baselineDays: number; baselineFileMb: number };
}

/** The Rust half's refusal as the page shows it, without the host's "bad arguments:" lead. */
export const message = (e: unknown) => (e instanceof Error ? e.message : String(e)).replace(/^bad arguments: /, "");

/** A whole number typed in a field; blank is null, which puts the default back. */
export const typedNumber = (text: string): number | null | undefined => {
  const t = text.replace(/[\s,_]/g, "");
  if (!t) return null;
  return /^\d+$/.test(t) ? Number(t) : undefined;
};

const group = (n: number) => n.toLocaleString("en-US");

/** A number field that saves on Enter or when it loses focus. */
function NumberRow({ label, value, unit, hint, save }: { label: string; value: number; unit: string; hint: string; save: (v: number | null) => void }) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  const commit = () => {
    const v = typedNumber(text);
    if (v === undefined) return save(Number.NaN);
    if (v !== value) save(v);
  };
  return (
    <div className="srow">
      <span>{label}</span>
      <input
        className="ao-input cl-num"
        aria-label={label}
        inputMode="numeric"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && commit()}
      />
      <span className="small ao-dim">{unit}</span>
      <span className="small ao-dim grow">{hint}</span>
    </div>
  );
}

export function SettingsPage({ bridge }: SettingsPageProps) {
  const [settings, setSettings] = useState<ClaudeSettings>();
  const [error, setError] = useState<string>();
  const [model, setModel] = useState("");
  const [tokens, setTokens] = useState("");

  useEffect(() => {
    let gone = false;
    bridge.call<ClaudeSettings>("settings").then(
      (s) => !gone && setSettings(s),
      (e: unknown) => !gone && setError(message(e)),
    );
    return () => {
      gone = true;
    };
  }, [bridge]);

  const save = (key: string, value: unknown) => {
    if (typeof value === "number" && Number.isNaN(value)) return void setError("Type a whole number, or leave it blank for the default.");
    void bridge.call<ClaudeSettings>("setSetting", { key, value }).then(
      (s) => {
        setError(undefined);
        setSettings(s);
      },
      (e: unknown) => setError(message(e)),
    );
  };

  if (!settings) return error ? <p className="small app-error">{error}</p> : null;
  const d = settings.defaults;
  const windows = Object.entries(settings.contextWindows);
  const putWindows = (next: Record<string, number>) => save("contextWindows", Object.keys(next).length ? next : null);
  const addWindow = () => {
    const n = typedNumber(tokens);
    if (!model.trim() || n == null) return setError("Type a model id and its window in tokens.");
    putWindows({ ...settings.contextWindows, [model.trim()]: n });
    setModel("");
    setTokens("");
  };
  return (
    <section aria-label="Claude settings" className="cl-settings">
      <NumberRow
        label="Meter threshold"
        value={settings.meterThreshold}
        unit="%"
        hint={`the context meter turns amber here; blank for ${d.meterThreshold}%`}
        save={(v) => save("meterThreshold", v)}
      />
      <h4 className="small cl-settings-head">Context windows</h4>
      <p className="small ao-dim">A model without one here counts {group(d.window)} tokens. An id covers its dated builds.</p>
      <ul className="ctx-list cl-windows" aria-label="Context windows">
        {windows.map(([id, n]) => (
          <li key={id} className="ctx-item cl-window" data-model={id}>
            <span className="cl-window-model">{id}</span>
            <span className="small grow">{group(n)} tokens</span>
            <button
              type="button"
              className="btn ghost"
              aria-label={`Remove ${id}`}
              onClick={() => putWindows(Object.fromEntries(windows.filter(([k]) => k !== id)))}
            >
              Remove
            </button>
          </li>
        ))}
      </ul>
      <div className="srow">
        <input className="ao-input" aria-label="Model id" placeholder="claude-opus-5" spellCheck={false} value={model} onChange={(e) => setModel(e.target.value)} />
        <input
          className="ao-input cl-num"
          aria-label="Window in tokens"
          placeholder="1000000"
          inputMode="numeric"
          value={tokens}
          onChange={(e) => setTokens(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && addWindow()}
        />
        <button type="button" className="btn" onClick={addWindow}>
          Add
        </button>
      </div>
      <h4 className="small cl-settings-head">Baselines</h4>
      <NumberRow
        label="Keep baselines for"
        value={settings.baselineDays}
        unit="days"
        hint={`after the session ends; blank for ${d.baselineDays}`}
        save={(v) => save("baselineDays", v)}
      />
      <NumberRow
        label="Total baseline storage"
        value={settings.baselineCapMb}
        unit="MB"
        hint={`oldest ended sessions go first; blank for ${d.baselineCapMb}`}
        save={(v) => save("baselineCapMb", v)}
      />
      <NumberRow
        label="Largest file kept"
        value={settings.baselineFileMb}
        unit="MB"
        hint={`a larger file shows no diff; blank for ${d.baselineFileMb}`}
        save={(v) => save("baselineFileMb", v)}
      />
      {error && (
        <p className="small app-error cl-settings-error" role="alert">
          {error}
        </p>
      )}
      <p className="small ao-dim">Baselines are copies of files as they were when a session started, kept only on this Mac.</p>
    </section>
  );
}

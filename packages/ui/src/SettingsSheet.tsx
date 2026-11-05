import {
  accelOf,
  ACTIONS,
  bindingProblem,
  chordLabel,
  conflicts,
  withBindings,
  type KeyBindings,
  COMMAND_TARGETS,
  MAX_TTL_SECS,
  VAR_MODES,
  type AppSettings,
  type AttentionSettings,
  type CommandTarget,
  type Host,
  type Id,
  type SavedCommand,
  type StatusConfig,
} from "@terminescu/host";
import { useEffect, useRef, useState } from "react";
import { MODE_HINT, MODE_LABEL } from "./ExtractDialog";
import { EXTRACTION_RULES, parseTtl, ttlText } from "./names";
import { StatusesSection } from "./StatusSettings";
import { TARGET_LABEL } from "./CommandsSection";

export const SETTINGS_SECTIONS = [
  { id: "statuses", label: "Statuses" },
  { id: "commands", label: "Commands" },
  { id: "variables", label: "Variables" },
  { id: "extraction", label: "Extraction" },
  { id: "shell", label: "Shell" },
  { id: "keys", label: "Keys" },
  { id: "notifications", label: "Notifications" },
] as const;
export type SettingsSection = (typeof SETTINGS_SECTIONS)[number]["id"];

type Api = Pick<
  Host,
  | "saveStatus"
  | "updateStatus"
  | "deleteStatus"
  | "reorderStatuses"
  | "addRule"
  | "toggleRule"
  | "deleteRule"
  | "setBuiltin"
  | "allCommands"
  | "updateCommand"
  | "deleteCommand"
  | "onCommands"
  | "settings"
  | "setSettings"
  | "onSettings"
  | "attentionSettings"
  | "setAttentionSettings"
  | "keyBindings"
  | "setKeyBindings"
  | "onKeyBindings"
  | "recordKeys"
>;

export interface SettingsSheetProps {
  section: SettingsSection;
  onSection(section: SettingsSection): void;
  statusConfig: StatusConfig;
  host: Api;
  /** A side tab's shown name, for commands that belong to one. */
  sideTabName(id: Id): string | undefined;
  onError(message: string): void;
  onClose(): void;
}

/** The Settings screen (D25): one sheet, a section list on the left. `⌘,` opens it. */
export function SettingsSheet(props: SettingsSheetProps) {
  const { section, onSection, onClose } = props;
  const frame = useRef<HTMLDivElement>(null);
  useEffect(() => frame.current?.focus(), []);
  const label = SETTINGS_SECTIONS.find((s) => s.id === section)!.label;
  return (
    <div className="sheet-back" onPointerDown={onClose}>
      <div
        ref={frame}
        tabIndex={-1}
        className="ao-frame status-settings settings-sheet"
        role="dialog"
        aria-labelledby="ss-t"
        onPointerDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            onClose();
          }
        }}
      >
        <div className="ao-frame-head">
          <i className="ao-i ao-i-dot" />
          <span id="ss-t" className="ao-pixel">
            Settings › {label}
          </span>
        </div>
        <div className="settings-split">
          <nav className="settings-nav" aria-label="Settings sections">
            {SETTINGS_SECTIONS.map((s) => (
              <button
                key={s.id}
                type="button"
                className={`settings-nav-item${s.id === section ? " active" : ""}`}
                aria-current={s.id === section ? "page" : undefined}
                onClick={() => onSection(s.id)}
              >
                {s.label}
              </button>
            ))}
          </nav>
          <div className="ao-frame-body status-body" data-section={section}>
            {section === "statuses" && <StatusesSection config={props.statusConfig} host={props.host} onError={props.onError} />}
            {section === "commands" && <CommandsSettings {...props} />}
            {section === "variables" && <VariableSettings {...props} />}
            {section === "extraction" && <ExtractionSettings />}
            {section === "shell" && <ShellSettings {...props} />}
            {section === "keys" && <KeysSettings {...props} />}
            {section === "notifications" && <NotificationSettings {...props} />}
          </div>
        </div>
        <div className="ao-frame-foot">
          <button type="button" className="ao-btn primary" onClick={onClose}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}

/** App settings, loaded once and again when another window changes them. */
function useAppSettings(host: Api, onError: (m: string) => void) {
  const [settings, setSettings] = useState<AppSettings>();
  useEffect(() => {
    let gone = false;
    let off: (() => void) | undefined;
    const load = () => void host.settings().then((s) => !gone && setSettings(s), (e: unknown) => onError(String(e)));
    load();
    void host.onSettings(load).then((o) => (gone ? o() : (off = o)));
    return () => {
      gone = true;
      off?.();
    };
  }, [host, onError]);
  const save = (patch: Partial<AppSettings>) => {
    if (!settings) return;
    void host.setSettings({ ...settings, ...patch }).then(setSettings, (e: unknown) => onError(String(e)));
  };
  return [settings, save] as const;
}

function CommandsSettings({ host, sideTabName, onError }: SettingsSheetProps) {
  const [commands, setCommands] = useState<SavedCommand[]>([]);
  useEffect(() => {
    let gone = false;
    let off: (() => void) | undefined;
    const load = () => void host.allCommands().then((c) => !gone && setCommands(c), (e: unknown) => onError(String(e)));
    load();
    void host.onCommands(load).then((o) => (gone ? o() : (off = o)));
    return () => {
      gone = true;
      off?.();
    };
  }, [host, onError]);
  return (
    <section aria-label="Commands">
      <h3 className="ao-pixel ao-dim">Commands</h3>
      {commands.length === 0 && <p className="small ao-dim">No commands yet. Add one with + in the sidebar's Commands section.</p>}
      {commands.map((c) => (
        <CommandEditRow
          key={c.id}
          command={c}
          scope={c.sideTabId === null ? "global" : (sideTabName(c.sideTabId) ?? "side tab")}
          host={host}
          onError={onError}
        />
      ))}
      <p className="small ao-dim">A launcher command runs in its own tab when its side tab opens and after a reboot.</p>
    </section>
  );
}

function CommandEditRow({ command: c, scope, host, onError }: { command: SavedCommand; scope: string; host: Api; onError(m: string): void }) {
  const [draft, setDraft] = useState({ name: c.name, body: c.body, target: c.target, launcher: c.launcher });
  useEffect(() => setDraft({ name: c.name, body: c.body, target: c.target, launcher: c.launcher }), [c]);
  const dirty = draft.name !== c.name || draft.body !== c.body || draft.target !== c.target || draft.launcher !== c.launcher;
  return (
    <form
      className="srow cmd-edit"
      data-command={c.name}
      onSubmit={(e) => {
        e.preventDefault();
        void host.updateCommand(c.id, { sideTabId: c.sideTabId, ...draft }).catch((err: unknown) => onError(String(err)));
      }}
    >
      <input className="ao-input" aria-label={`${c.name} name`} value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
      <input className="ao-input grow" aria-label={`${c.name} command`} value={draft.body} onChange={(e) => setDraft({ ...draft, body: e.target.value })} />
      <select className="ao-input" aria-label={`${c.name} runs in`} value={draft.target} onChange={(e) => setDraft({ ...draft, target: e.target.value as CommandTarget })}>
        {COMMAND_TARGETS.map((t) => (
          <option key={t} value={t}>
            {TARGET_LABEL[t]}
          </option>
        ))}
      </select>
      <label className="small" title={c.sideTabId === null ? "A global command has no side tab to launch in" : undefined}>
        <input type="checkbox" disabled={c.sideTabId === null} checked={draft.launcher} onChange={(e) => setDraft({ ...draft, launcher: e.target.checked })} /> launcher
      </label>
      <span className="small ao-dim cmd-scope">{scope}</span>
      <button type="submit" className="ao-btn" disabled={!dirty}>
        Save
      </button>
      <button type="button" className="ao-btn ghost" aria-label={`Delete ${c.name}`} onClick={() => void host.deleteCommand(c.id).catch((e: unknown) => onError(String(e)))}>
        ×
      </button>
    </form>
  );
}

function VariableSettings({ host, onError }: SettingsSheetProps) {
  const [settings, save] = useAppSettings(host, onError);
  const [ttl, setTtl] = useState("");
  useEffect(() => setTtl(settings?.varTtlSecs ? ttlText(settings.varTtlSecs) : ""), [settings?.varTtlSecs]);
  if (!settings) return null;
  const commitTtl = () => {
    if (!ttl.trim()) return settings.varTtlSecs !== null && save({ varTtlSecs: null });
    const secs = parseTtl(ttl);
    if (secs === undefined || secs > MAX_TTL_SECS) {
      onError("A TTL reads like 30m, 1h or 2d, at most 30d.");
      return setTtl(settings.varTtlSecs ? ttlText(settings.varTtlSecs) : "");
    }
    if (secs !== settings.varTtlSecs) save({ varTtlSecs: secs });
  };
  return (
    <section aria-label="Variable defaults">
      <h3 className="ao-pixel ao-dim">New variables start as</h3>
      <div role="radiogroup" aria-label="Default storage mode">
        {VAR_MODES.map((m) => (
          <label key={m} className="srow">
            <input type="radio" name="var-mode" checked={settings.varMode === m} onChange={() => save({ varMode: m })} />
            <span>{MODE_LABEL[m]}</span>
            <span className="small ao-dim grow">{MODE_HINT[m]}</span>
          </label>
        ))}
      </div>
      <div className="srow">
        <span>TTL for session and secret</span>
        <input
          className="ao-input"
          aria-label="Default TTL"
          placeholder="none"
          value={ttl}
          onChange={(e) => setTtl(e.target.value)}
          onBlur={commitTtl}
          onKeyDown={(e) => e.key === "Enter" && commitTtl()}
        />
        <span className="small ao-dim grow">30m, 1h or 2d; empty keeps them until their side tab ends</span>
      </div>
      <p className="small ao-dim">Extract starts with these; you can still change them there. A saved variable has no TTL.</p>
    </section>
  );
}

function ExtractionSettings() {
  return (
    <section aria-label="Extraction rules">
      <h3 className="ao-pixel ao-dim">Extraction rules</h3>
      {EXTRACTION_RULES.map((r) => (
        <div key={r.what} className="srow" data-rule-name={r.what}>
          <span className="set-what">{r.what}</span>
          <code className="grow set-example">{r.example}</code>
          <span className="small ao-dim">named {r.name}</span>
        </div>
      ))}
      <p className="small ao-dim">Read-only in this version. Anything else extracted is named text.</p>
    </section>
  );
}

function ShellSettings({ host, onError }: SettingsSheetProps) {
  const [settings] = useAppSettings(host, onError);
  if (!settings) return null;
  return (
    <section aria-label="Shell">
      <h3 className="ao-pixel ao-dim">Shell</h3>
      <div className="srow">
        <span>New tabs</span>
        <span className="small ao-dim grow">start your login shell in the focused pane's folder, never claude</span>
      </div>
      <label className="srow">
        <input type="checkbox" disabled checked={settings.editor} aria-describedby="editor-hint" />
        <span>
          Set <code>VISUAL</code> and <code>EDITOR</code> to <code>terminescu edit --wait</code>
        </span>
        <span id="editor-hint" className="small ao-dim grow">
          arrives with text tabs: git and Claude Code then open their editor in a pane beside yours
        </span>
      </label>
    </section>
  );
}

function KeysSettings({ host, onError }: SettingsSheetProps) {
  const [bindings, setBindings] = useState<KeyBindings>({});
  const [recording, setRecording] = useState<string>();
  const [problem, setProblem] = useState<{ id: string; text: string }>();
  useEffect(() => {
    let gone = false;
    let off: (() => void) | undefined;
    const load = () => void host.keyBindings().then((b) => !gone && setBindings(b), (e: unknown) => onError(String(e)));
    load();
    void host.onKeyBindings(load).then((o) => (gone ? o() : (off = o)));
    return () => {
      gone = true;
      off?.();
    };
  }, [host, onError]);
  // The menu gives up its chords while recording, and gets them back however recording ends.
  useEffect(() => {
    if (!recording) return;
    void host.recordKeys(true).catch((e: unknown) => onError(String(e)));
    return () => void host.recordKeys(false).catch((e: unknown) => onError(String(e)));
  }, [host, onError, recording]);
  const actions = withBindings(ACTIONS, bindings);
  const clashing = conflicts(actions);
  const save = (next: KeyBindings) => void host.setKeyBindings(next).then(setBindings, (e: unknown) => onError(String(e)));

  // The next chord goes to the recorder wherever focus is (WebKit does not focus a clicked
  // button), ahead of the sheet's Escape and the panes.
  const latest = useRef({ actions, bindings });
  latest.current = { actions, bindings };
  useEffect(() => {
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape" && !e.metaKey) return setRecording(undefined);
      const accel = accelOf(e);
      if (!accel) return;
      const { actions, bindings } = latest.current;
      const why = bindingProblem(recording, accel, actions);
      if (why) return setProblem({ id: recording, text: why });
      setProblem(undefined);
      setRecording(undefined);
      save({ ...bindings, [recording]: accel });
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
    // `save` reads only host and onError, both in the deps.
  }, [host, onError, recording]);

  return (
    <section aria-label="Keys">
      <h3 className="ao-pixel ao-dim">Keys</h3>
      {actions.map((a) => (
        <div key={a.id} className={`srow${clashing.has(a.id) ? " key-clash" : ""}`} data-action={a.id}>
          <span className="grow">{a.label}</span>
          {problem?.id === a.id && (
            <span className="small key-problem" role="alert">
              {problem.text}
            </span>
          )}
          <span className="small ao-dim">{a.menu}</span>
          {bindings[a.id] && (
            <button
              type="button"
              className="link small"
              aria-label={`Reset ${a.label}`}
              onClick={() => {
                const { [a.id]: _, ...rest } = bindings;
                save(rest);
              }}
            >
              reset
            </button>
          )}
          <button
            type="button"
            className={`ao-btn ghost set-chord${recording === a.id ? " recording" : ""}`}
            aria-label={`Shortcut for ${a.label}`}
            onClick={() => {
              setProblem(undefined);
              setRecording(recording === a.id ? undefined : a.id);
            }}
          >
            {recording === a.id ? "press a chord" : <kbd>{chordLabel(a.accel)}</kbd>}
          </button>
        </div>
      ))}
      <div className="srow">
        <span className="small ao-dim grow">Click a shortcut, then press the new ⌘ chord; Esc cancels. Plain keys always reach the focused terminal.</span>
        {Object.keys(bindings).length > 0 && (
          <button type="button" className="ao-btn ghost" onClick={() => save({})}>
            Reset all
          </button>
        )}
      </div>
    </section>
  );
}

function NotificationSettings({ host, onError }: SettingsSheetProps) {
  const [attention, setAttention] = useState<AttentionSettings>();
  useEffect(() => {
    void host.attentionSettings().then(setAttention, (e: unknown) => onError(String(e)));
  }, [host, onError]);
  if (!attention) return null;
  const save = (patch: Partial<AttentionSettings>) => {
    const next = { ...attention, ...patch };
    void host.setAttentionSettings(next).then(() => setAttention(next), (e: unknown) => onError(String(e)));
  };
  return (
    <section aria-label="Notifications">
      <h3 className="ao-pixel ao-dim">When Claude waits for you</h3>
      <label className="srow">
        <input type="checkbox" checked={attention.notify} onChange={(e) => save({ notify: e.target.checked })} />
        <span>macOS notification</span>
        <span className="small ao-dim grow">only while Terminescu is in the background, no sound</span>
      </label>
      <label className="srow">
        <input type="checkbox" checked={attention.chime} onChange={(e) => save({ chime: e.target.checked })} />
        <span>Chime</span>
        <span className="small ao-dim grow">a short tone in the window that holds the session</span>
      </label>
      <p className="small ao-dim">The dock badge and the amber marker always show.</p>
    </section>
  );
}

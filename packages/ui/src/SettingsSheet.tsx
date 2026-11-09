import {
  accelOf,
  ACTIONS,
  bindingProblem,
  chordLabel,
  conflicts,
  normalAccel,
  RESERVED,
  TERMINAL_KEYS,
  terminalChordProblem,
  withBindings,
  type KeyBindings,
  COMMAND_TARGETS,
  DEFAULT_SETTINGS,
  FONT_FAMILY_MAX,
  FONT_NAME,
  FONT_SIZES,
  LETTER_SPACINGS,
  LINE_HEIGHTS,
  MAX_TTL_SECS,
  SCROLLBACKS,
  OPACITIES,
  COMMAND_SECS,
  TAB_FONT_SIZES,
  VAR_MODES,
  type ActionDef,
  type AppSettings,
  type TerminalKeys,
  type AttentionSettings,
  type Bell,
  type CursorStyle,
  type TerminalProfile,
  type TerminalTheme,
  type OptionMeta,
  type CommandTarget,
  type Host,
  type Id,
  type SavedCommand,
  type StatusConfig,
} from "@terminescu/host";
import { useEffect, useRef, useState } from "react";
import { MODE_HINT, MODE_LABEL } from "./ExtractDialog";
import { EXTRACTION_RULES, parseTtl, ttlText } from "./names";
import { ANSI_SLOTS, schemeTheme, terminalFontOptions } from "./xterm-theme";
import { StatusesSection } from "./StatusSettings";
import { TARGET_LABEL } from "./CommandsSection";

export const SETTINGS_SECTIONS = [
  { id: "statuses", label: "Statuses" },
  { id: "commands", label: "Commands" },
  { id: "variables", label: "Variables" },
  { id: "extraction", label: "Extraction" },
  { id: "terminal", label: "Terminal" },
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
  | "importTerminalApp"
  | "importTerminalFile"
  | "chooseFile"
>;

const THEME_LABEL: Record<TerminalTheme, string> = {
  terminescu: "Terminescu",
  basic: "Terminal.app Basic",
  solarizedDark: "Solarized Dark",
  solarizedLight: "Solarized Light",
  imported: "Imported",
};

/** What an imported profile changes, as a Settings patch and a line naming it (task 8.47). */
export function profilePatch(p: TerminalProfile): { patch: Partial<AppSettings>; note: string } {
  const patch: Partial<AppSettings> = {};
  const took: string[] = [];
  if (p.colors) {
    patch.terminalColors = p.colors;
    patch.terminalTheme = "imported";
    took.push("colours");
  }
  if (p.fontFamily != null && p.fontFamily.length <= FONT_FAMILY_MAX && FONT_NAME.test(p.fontFamily)) {
    patch.terminalFont = p.fontFamily;
    took.push(p.fontFamily || "SF Mono");
  }
  if (p.fontSize != null) {
    patch.terminalFontSize = Math.min(FONT_SIZES[1], Math.max(FONT_SIZES[0], Math.round(p.fontSize)));
    took.push(`${patch.terminalFontSize} px`);
  }
  if (p.cursor) {
    patch.terminalCursor = p.cursor;
    took.push(`${p.cursor} cursor`);
  }
  if (p.cursorBlink != null) patch.terminalCursorBlink = p.cursorBlink;
  if (p.optionAsMeta != null) patch.optionAsMeta = p.optionAsMeta ? "both" : "off";
  if (p.backgroundOpacity != null) {
    patch.terminalOpacity = Math.min(OPACITIES[1], Math.max(OPACITIES[0], Math.round(p.backgroundOpacity)));
    took.push(`${patch.terminalOpacity}% opacity`);
  }
  return { patch, note: took.length ? `Imported ${p.name}: ${took.join(", ")}.` : `${p.name} has nothing Terminescu can use.` };
}

export interface SettingsSheetProps {
  section: SettingsSection;
  onSection(section: SettingsSection): void;
  statusConfig: StatusConfig;
  host: Api;
  /** A side tab's shown name, for commands that belong to one. */
  sideTabName(id: Id): string | undefined;
  onError(message: string): void;
  onClose(): void;
  /** Deletes a command, with an undo; without it the command goes at once. */
  onDeleteCommand?(command: SavedCommand): Promise<unknown>;
  /** Commands deleted but still waiting on their undo, left out of the list. */
  hiddenCommands?: readonly string[];
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
        aria-modal="true"
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
            {section === "terminal" && <TerminalSettings {...props} />}
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

function CommandsSettings({ host, sideTabName, onError, onDeleteCommand, hiddenCommands = [] }: SettingsSheetProps) {
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
  const shown = commands.filter((c) => !hiddenCommands.includes(c.id));
  const remove = (c: SavedCommand) => (onDeleteCommand ? onDeleteCommand(c) : host.deleteCommand(c.id));
  return (
    <section aria-label="Commands">
      <h3 className="ao-pixel ao-dim">Commands</h3>
      {shown.length === 0 && <p className="small ao-dim">No commands yet. Add one with + in the sidebar's Commands section.</p>}
      {shown.map((c) => (
        <CommandEditRow
          key={c.id}
          command={c}
          scope={c.sideTabId === null ? "global" : (sideTabName(c.sideTabId) ?? "side tab")}
          host={host}
          onError={onError}
          onDelete={() => remove(c)}
        />
      ))}
      <p className="small ao-dim">A launcher command runs in its own tab when its side tab opens and after a reboot.</p>
    </section>
  );
}

function CommandEditRow({
  command: c,
  scope,
  host,
  onError,
  onDelete,
}: {
  command: SavedCommand;
  scope: string;
  host: Api;
  onError(m: string): void;
  onDelete(): Promise<unknown>;
}) {
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
      <button type="button" className="ao-btn ghost" aria-label={`Delete ${c.name}`} onClick={() => void onDelete().catch((e: unknown) => onError(String(e)))}>
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

/** Monospaced families macOS ships or people often install; any installed name works. */
const FONT_SUGGESTIONS = ["SF Mono", "Menlo", "Monaco", "Courier New", "JetBrains Mono", "Fira Code", "Cascadia Code", "Source Code Pro", "Hack", "IBM Plex Mono"];
const WEIGHTS: [number, string][] = [
  [100, "Thin"],
  [200, "Extra light"],
  [300, "Light"],
  [400, "Regular"],
  [500, "Medium"],
  [600, "Semibold"],
  [700, "Bold"],
  [800, "Heavy"],
  [900, "Black"],
];
const PREVIEW = "~/work/acme-api main ❯ ls -la\ndrwxr-xr-x  12 owner  staff   384 src\n-rw-r--r--   1 owner  staff  2.1K README.md\n0O 1lI {}[] => != <= ~~ //";

/** A whole number field that saves each value inside its range and puts back anything else on blur. */
function NumberField({ label, value, range, onSave }: { label: string; value: number; range: readonly [number, number]; onSave(v: number): void }) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  const valid = (t: string) => /^-?\d+$/.test(t.trim()) && Number(t) >= range[0] && Number(t) <= range[1];
  return (
    <input
      className="ao-input set-num"
      type="number"
      aria-label={label}
      min={range[0]}
      max={range[1]}
      step={1}
      value={text}
      onChange={(e) => {
        setText(e.target.value);
        if (valid(e.target.value) && Number(e.target.value) !== value) onSave(Number(e.target.value));
      }}
      onBlur={() => !valid(text) && setText(String(value))}
    />
  );
}

/** Settings › Terminal (task 8.36): the terminal's font and spacing, and the tab text size. */
function TerminalSettings({ host, onError }: SettingsSheetProps) {
  const [settings, save] = useAppSettings(host, onError);
  const [family, setFamily] = useState("");
  const [imported, setImported] = useState<string>();
  useEffect(() => setFamily(settings?.terminalFont ?? ""), [settings?.terminalFont]);
  if (!settings) return null;
  const commitFamily = () => {
    const name = family.trim();
    if (name.length > FONT_FAMILY_MAX || !FONT_NAME.test(name)) {
      onError("A font name has letters, digits, spaces, dots, dashes and underscores.");
      return setFamily(settings.terminalFont);
    }
    if (name !== settings.terminalFont) save({ terminalFont: name });
  };
  const font = terminalFontOptions(settings);
  const lh = settings.terminalLineHeight;
  const scheme = schemeTheme(settings.terminalTheme, settings.terminalColors);
  const apply = (p: TerminalProfile) => {
    const { patch, note } = profilePatch(p);
    setImported(note);
    if (Object.keys(patch).length) save(patch);
  };
  return (
    <section aria-label="Terminal">
      <h3 className="ao-pixel ao-dim">Terminal font</h3>
      <div className="srow">
        <span>Font</span>
        <input
          className="ao-input"
          aria-label="Terminal font"
          list="terminal-fonts"
          placeholder="SF Mono"
          value={family}
          onChange={(e) => setFamily(e.target.value)}
          onBlur={commitFamily}
          onKeyDown={(e) => e.key === "Enter" && commitFamily()}
        />
        <datalist id="terminal-fonts">
          {FONT_SUGGESTIONS.map((f) => (
            <option key={f} value={f} />
          ))}
        </datalist>
        <span className="small ao-dim grow">empty is SF Mono; a font that is not installed falls back to it</span>
      </div>
      <div className="srow">
        <span>Size</span>
        <NumberField label="Font size" value={settings.terminalFontSize} range={FONT_SIZES} onSave={(v) => save({ terminalFontSize: v })} />
        <span className="small ao-dim grow">px, {FONT_SIZES[0]} to {FONT_SIZES[1]}; ⌘+ and ⌘- zoom everything on top</span>
      </div>
      <div className="srow">
        <span>Line height</span>
        <input
          type="range"
          aria-label="Line height"
          min={LINE_HEIGHTS[0]}
          max={LINE_HEIGHTS[1]}
          step={5}
          value={lh}
          onChange={(e) => save({ terminalLineHeight: Number(e.target.value) })}
        />
        <output className="set-out">{(lh / 100).toFixed(2)}</output>
        <span className="small ao-dim grow">1.00 fits the most lines</span>
      </div>
      <div className="srow">
        <span>Letter spacing</span>
        <NumberField label="Letter spacing" value={settings.terminalLetterSpacing} range={LETTER_SPACINGS} onSave={(v) => save({ terminalLetterSpacing: v })} />
        <span className="small ao-dim grow">px between characters</span>
      </div>
      <div className="srow">
        <span>Weight</span>
        <select className="ao-input" aria-label="Font weight" value={settings.terminalFontWeight} onChange={(e) => save({ terminalFontWeight: Number(e.target.value) })}>
          {WEIGHTS.map(([w, name]) => (
            <option key={w} value={w}>
              {name}
            </option>
          ))}
        </select>
        <span className="small ao-dim grow">bold text is three steps heavier</span>
      </div>
      <pre
        className="term-preview"
        aria-label="Preview"
        style={{
          fontFamily: font.fontFamily,
          fontSize: font.fontSize,
          lineHeight: font.lineHeight,
          letterSpacing: font.letterSpacing,
          fontWeight: font.fontWeight,
        }}
      >
        {PREVIEW}
      </pre>
      <h3 className="ao-pixel ao-dim">Colours</h3>
      <div className="srow">
        <span>Theme</span>
        <select
          className="ao-input"
          aria-label="Colour theme"
          value={settings.terminalTheme}
          onChange={(e) => save({ terminalTheme: e.target.value as TerminalTheme })}
        >
          {(Object.keys(THEME_LABEL) as TerminalTheme[])
            .filter((t) => t !== "imported" || settings.terminalColors)
            .map((t) => (
              <option key={t} value={t}>
                {THEME_LABEL[t]}
              </option>
            ))}
        </select>
        <span className="term-swatches" aria-label="Theme colours">
          {[scheme.background, scheme.foreground, ...ANSI_SLOTS.map((s) => scheme[s])].map((c, i) => (
            <span key={i} style={{ background: c as string }} />
          ))}
        </span>
        <span className="small ao-dim grow">a status background still tints the terminals</span>
      </div>
      <div className="srow">
        <span>Import</span>
        <button type="button" className="ao-btn ghost" onClick={() => void host.importTerminalApp().then(apply, (e: unknown) => onError(String(e)))}>
          From Terminal.app
        </button>
        <button
          type="button"
          className="ao-btn ghost"
          onClick={() =>
            void host
              .chooseFile()
              .then((path) => (path ? host.importTerminalFile(path).then(apply) : undefined))
              .catch((e: unknown) => onError(String(e)))
          }
        >
          File…
        </button>
        <span className="small ao-dim grow" role="status">
          {imported ?? "the default profile, or a .terminal or .itermcolors file"}
        </span>
      </div>
      <div className="srow">
        <span>Opacity</span>
        <input
          type="range"
          aria-label="Terminal opacity"
          min={OPACITIES[0]}
          max={OPACITIES[1]}
          step={5}
          value={settings.terminalOpacity}
          onChange={(e) => save({ terminalOpacity: Number(e.target.value) })}
        />
        <output className="set-out">{settings.terminalOpacity}%</output>
        <span className="small ao-dim grow">below 100% the desktop shows through the terminals; the sidebar and bars stay solid</span>
      </div>
      <h3 className="ao-pixel ao-dim">Terminal behaviour</h3>
      <div className="srow">
        <span>Cursor</span>
        <select className="ao-input" aria-label="Cursor" value={settings.terminalCursor} onChange={(e) => save({ terminalCursor: e.target.value as CursorStyle })}>
          <option value="bar">Bar</option>
          <option value="block">Block</option>
          <option value="underline">Underline</option>
        </select>
        <label className="small">
          <input type="checkbox" checked={settings.terminalCursorBlink} onChange={(e) => save({ terminalCursorBlink: e.target.checked })} /> blink
        </label>
        <span className="small ao-dim grow">an unfocused terminal shows an outline</span>
      </div>
      <div className="srow">
        <span>History</span>
        <NumberField label="History lines" value={settings.terminalScrollback} range={SCROLLBACKS} onSave={(v) => save({ terminalScrollback: v })} />
        <span className="small ao-dim grow">lines, {SCROLLBACKS[0].toLocaleString("en-US")} to {SCROLLBACKS[1].toLocaleString("en-US")}; a reattach brings back what new shells keep</span>
      </div>
      <div className="srow">
        <span>Option as Meta</span>
        <select className="ao-input" aria-label="Option as Meta" value={settings.optionAsMeta} onChange={(e) => save({ optionAsMeta: e.target.value as OptionMeta })}>
          <option value="both">Both Option keys</option>
          <option value="left">Left Option only</option>
          <option value="right">Right Option only</option>
          <option value="off">Off</option>
        </select>
        <span className="small ao-dim grow">Meta sends Esc first, so ⌥↵ is a newline in Claude Code; the other key types ∑ or é</span>
      </div>
      <div className="srow">
        <span>Bell</span>
        <select className="ao-input" aria-label="Bell" value={settings.bell} onChange={(e) => save({ bell: e.target.value as Bell })}>
          <option value="off">Off</option>
          <option value="visual">Flash</option>
          <option value="sound">Sound</option>
        </select>
        <span className="small ao-dim grow">flash and sound also bounce the dock icon while Terminescu is in the background</span>
      </div>
      <label className="srow">
        <input type="checkbox" checked={settings.copyOnSelect} onChange={(e) => save({ copyOnSelect: e.target.checked })} />
        <span>Copy on select</span>
      </label>
      <label className="srow">
        <input type="checkbox" checked={settings.pasteWarning} onChange={(e) => save({ pasteWarning: e.target.checked })} />
        <span>Ask before a risky paste</span>
        <span className="small ao-dim grow">over 5,000 characters, a sudo line, or a pipe into a shell</span>
      </label>
      <label className="srow">
        <input type="checkbox" checked={settings.raiseContrast} onChange={(e) => save({ raiseContrast: e.target.checked })} />
        <span>Raise low contrast</span>
        <span className="small ao-dim grow">text too close to its background is drawn lighter or darker</span>
      </label>
      <h3 className="ao-pixel ao-dim">Tabs</h3>
      <label className="srow">
        <input type="checkbox" checked={settings.topTabs} onChange={(e) => save({ topTabs: e.target.checked })} />
        <span>Show tabs in the top bar</span>
        <span className="small ao-dim grow">tabs always show in the sidebar</span>
      </label>
      <div className="srow">
        <span>Tab text</span>
        <NumberField label="Tab text size" value={settings.tabFontSize} range={TAB_FONT_SIZES} onSave={(v) => save({ tabFontSize: v })} />
        <span className="small ao-dim grow">px, {TAB_FONT_SIZES[0]} to {TAB_FONT_SIZES[1]}</span>
      </div>
      <div className="srow">
        <button
          type="button"
          className="ao-btn ghost"
          onClick={() =>
            save({
              terminalFont: DEFAULT_SETTINGS.terminalFont,
              terminalFontSize: DEFAULT_SETTINGS.terminalFontSize,
              terminalLineHeight: DEFAULT_SETTINGS.terminalLineHeight,
              terminalLetterSpacing: DEFAULT_SETTINGS.terminalLetterSpacing,
              terminalFontWeight: DEFAULT_SETTINGS.terminalFontWeight,
              tabFontSize: DEFAULT_SETTINGS.tabFontSize,
              terminalCursor: DEFAULT_SETTINGS.terminalCursor,
              terminalCursorBlink: DEFAULT_SETTINGS.terminalCursorBlink,
              terminalScrollback: DEFAULT_SETTINGS.terminalScrollback,
              optionAsMeta: DEFAULT_SETTINGS.optionAsMeta,
              bell: DEFAULT_SETTINGS.bell,
              copyOnSelect: DEFAULT_SETTINGS.copyOnSelect,
              pasteWarning: DEFAULT_SETTINGS.pasteWarning,
              raiseContrast: DEFAULT_SETTINGS.raiseContrast,
              terminalTheme: DEFAULT_SETTINGS.terminalTheme,
              terminalOpacity: DEFAULT_SETTINGS.terminalOpacity,
            })
          }
        >
          Reset to defaults
        </button>
        <span className="small ao-dim grow">every open terminal changes at once</span>
      </div>
    </section>
  );
}

function ShellSettings({ host, onError }: SettingsSheetProps) {
  const [settings, change] = useAppSettings(host, onError);
  if (!settings) return null;
  return (
    <section aria-label="Shell">
      <h3 className="ao-pixel ao-dim">Shell</h3>
      <div className="srow">
        <span>New tabs</span>
        <span className="small ao-dim grow">start your login shell in the focused pane's folder, never claude</span>
      </div>
      <label className="srow">
        <input type="checkbox" checked={settings.editor} aria-describedby="editor-hint" onChange={(e) => change({ editor: e.target.checked })} />
        <span>
          Set <code>VISUAL</code> and <code>EDITOR</code> to <code>terminescu edit --wait</code>
        </span>
        <span id="editor-hint" className="small ao-dim grow">
          new shells only: git and Claude Code then open their editor in a pane beside yours, ⌘↵ returns
        </span>
      </label>
      <label className="srow">
        <input
          type="checkbox"
          checked={settings.hook === "on"}
          aria-describedby="hook-hint"
          onChange={(e) => change({ hook: e.target.checked ? "on" : "off", firstRunDone: true })}
        />
        <span>Claude Code hook</span>
        <span id="hook-hint" className="small ao-dim grow">
          Claude Code tells Terminescu when a session starts, waits or ends; off removes only Terminescu&apos;s entry from Claude Code&apos;s settings
        </span>
      </label>
    </section>
  );
}

/** Recorder ids for the terminal's chords, apart from the app actions' ids. */
const TERM = "term:";

/** Why `accel` cannot be terminal key `id`'s chord: a menu chord would never reach the pane. */
function terminalKeyProblem(id: keyof TerminalKeys, accel: string, keys: TerminalKeys, actions: readonly ActionDef[]): string | undefined {
  const why = terminalChordProblem(accel);
  if (why) return why;
  const want = normalAccel(accel);
  const reserved = RESERVED.find((r) => normalAccel(r.accel) === want);
  if (reserved) return `${chordLabel(accel)} is ${reserved.label}.`;
  const action = actions.find((a) => normalAccel(a.accel) === want);
  if (action) return `${chordLabel(accel)} is already ${action.label}.`;
  const other = TERMINAL_KEYS.find((k) => k.id !== id && keys[k.id] && normalAccel(keys[k.id]) === want);
  if (other) return `${chordLabel(accel)} is already ${other.label}.`;
  return undefined;
}

function KeysSettings({ host, onError }: SettingsSheetProps) {
  const [settings, change] = useAppSettings(host, onError);
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
  const termKeys = settings?.terminalKeys ?? DEFAULT_SETTINGS.terminalKeys;
  const latest = useRef({ actions, bindings, termKeys, change });
  latest.current = { actions, bindings, termKeys, change };
  useEffect(() => {
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape" && !e.metaKey) return setRecording(undefined);
      const accel = accelOf(e);
      if (!accel) return;
      const { actions, bindings, termKeys, change } = latest.current;
      if (recording.startsWith(TERM)) {
        const id = recording.slice(TERM.length) as keyof TerminalKeys;
        const why = terminalKeyProblem(id, accel, termKeys, actions);
        if (why) return setProblem({ id: recording, text: why });
        setProblem(undefined);
        setRecording(undefined);
        return change({ terminalKeys: { ...termKeys, [id]: accel } });
      }
      const why = bindingProblem(recording, accel, actions);
      if (why) return setProblem({ id: recording, text: why });
      setProblem(undefined);
      setRecording(undefined);
      save({ ...bindings, [recording]: accel });
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
    // `save` reads only host and onError, both in the deps; the rest comes from `latest`.
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
      <h3 className="ao-pixel ao-dim">In the terminal</h3>
      {settings &&
        TERMINAL_KEYS.map((k) => {
          const rid = TERM + k.id;
          const accel = termKeys[k.id];
          const changed = accel !== DEFAULT_SETTINGS.terminalKeys[k.id];
          // A chord saved before an app shortcut took it never reaches the pane.
          const taken = accel ? actions.find((a) => normalAccel(a.accel) === normalAccel(accel)) : undefined;
          return (
            <div key={k.id} className={`srow${taken ? " key-clash" : ""}`} data-term-key={k.id}>
              <span className="grow">{k.label}</span>
              {problem?.id === rid ? (
                <span className="small key-problem" role="alert">
                  {problem.text}
                </span>
              ) : (
                taken && (
                  <span className="small key-problem" role="alert">
                    {chordLabel(accel)} is {taken.label}, which wins.
                  </span>
                )
              )}
              {changed && (
                <button
                  type="button"
                  className="link small"
                  aria-label={`Reset ${k.label}`}
                  onClick={() => change({ terminalKeys: { ...termKeys, [k.id]: DEFAULT_SETTINGS.terminalKeys[k.id] } })}
                >
                  reset
                </button>
              )}
              {accel && (
                <button type="button" className="link small" aria-label={`Turn off ${k.label}`} onClick={() => change({ terminalKeys: { ...termKeys, [k.id]: "" } })}>
                  off
                </button>
              )}
              <button
                type="button"
                className={`ao-btn ghost set-chord${recording === rid ? " recording" : ""}`}
                aria-label={`Shortcut for ${k.label}`}
                onClick={() => {
                  setProblem(undefined);
                  setRecording(recording === rid ? undefined : rid);
                }}
              >
                {recording === rid ? "press a chord" : accel ? <kbd>{chordLabel(accel)}</kbd> : <span className="ao-dim">off</span>}
              </button>
            </div>
          );
        })}
      <div className="srow">
        <span className="small ao-dim grow">
          A focused terminal turns these into the keys a shell reads. The new line chord sends what ⌥↵ sends, a line break in Claude&apos;s prompt; ⇧↵ is a common pick.
        </span>
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
      <h3 className="ao-pixel ao-dim">When a long command ends</h3>
      <label className="srow">
        <input type="checkbox" checked={attention.commandNotify} onChange={(e) => save({ commandNotify: e.target.checked })} />
        <span>macOS notification</span>
        <span className="small ao-dim grow">while Terminescu is in the background, with the command, its time and a failed exit</span>
      </label>
      <div className="srow">
        <span>Long means</span>
        <NumberField label="Long command seconds" value={attention.commandSecs} range={COMMAND_SECS} onSave={(v) => save({ commandSecs: v })} />
        <span className="small ao-dim grow">
          seconds or more, {COMMAND_SECS[0]} to {COMMAND_SECS[1]}; panes running Claude use the waiting notification
        </span>
      </div>
    </section>
  );
}

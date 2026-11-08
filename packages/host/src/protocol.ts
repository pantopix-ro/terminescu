// The Host protocol (D6). Mirrors crates/core/src/state.rs; fixtures/host-protocol.json is
// checked by tests on both sides, so a field renamed on one side fails the other's tests.

export type Id = string;

export type Dir = "row" | "column";

/** A tab's split tree (D3). Sizes are thousandths and add up to 1000. */
export type Layout =
  | {
      type: "pane";
      id: Id;
      /** A Claude session to resume after a reboot (D23). */
      resume?: string;
      /** Set on a transcript pane: the Claude session it mirrors; such a pane has no shell (task 4.4). */
      transcript?: string;
      /** Set on a text pane: the document it edits; such a pane has no shell (D27). */
      doc?: Id;
    }
  | { type: "split"; id: Id; dir: Dir; children: Layout[]; sizes: number[] };

export interface Tab {
  id: Id;
  title: string | null;
  layout: Layout;
  /** Set by hand in the status editor; absent means the generated status shows (D15). */
  status?: ManualStatus;
  /** One of {@link ICONS}; absent shows the default icon (task 8.16). */
  icon?: Icon;
  /** A neon given when the tab is made (task 8.26): its icon and its square in the sidebar. */
  accent?: Neon;
}

/** A terminal pane's live label (task 8.30): where it is and what runs in it. */
export interface PaneLabel {
  /** The working directory, `~`-relative under home. */
  path?: string;
  /** The running program's name, never its arguments. */
  program?: string;
  /** Claude's terminal title, without its spinner, while Claude runs. */
  title?: string;
  /** The pty's foreground process, `zsh` at the prompt, as Terminal.app names a tab (task 8.62). */
  process?: string;
}

export interface SideTab {
  id: Id;
  title: string | null;
  tabs: Tab[];
  status?: ManualStatus;
  icon?: Icon;
  /** A neon given when the row is made (task 8.25): its edge and icon take `--neon-*`. */
  accent?: Neon;
  /** A random animal or bird, unique across windows (task 8.60). It names the row while the row holds two tabs or more; `title` overrides it. */
  name?: string;
}

/** Neon row accents, as `--neon-*` tokens. Mirrors `NEONS` in state.rs; no amber. */
export const NEONS = ["cyan", "pink", "lime", "violet", "mint", "blue", "rose", "ice"] as const;
export type Neon = (typeof NEONS)[number];

/** The icons a side tab or tab can pick; each is an `ao-i-*` class. Mirrors `ICONS` in state.rs. */
export const ICONS = [
  "term", "agent", "group", "tab", "window", "split", "doc", "tpl", "prompt", "var", "link",
  "pin", "play", "check", "bp", "pr", "merge", "save", "in", "out",
] as const;
export type Icon = (typeof ICONS)[number];

/** Accent hues a status may take; amber stays reserved for "needs you" (D16). */
export const ACCENTS = ["blue", "cyan", "green", "magenta", "red", "fg"] as const;
export type Accent = (typeof ACCENTS)[number];
/** Background tints a status may take. */
export const TINTS = ["blue", "cyan", "green", "magenta", "red"] as const;
export type Tint = (typeof TINTS)[number];
/** Longest status label, in characters. Same as `MAX_LABEL` in crates/core/src/status.rs. */
export const MAX_LABEL = 24;

/** How a status looks. */
export interface Look {
  label: string;
  accent: Accent | null;
  background: Tint | null;
}

/** A status set by hand; `statusId` links it to a saved status, so edits there restyle it. */
export interface ManualStatus extends Look {
  statusId?: Id;
}

/** The built-in statuses, least urgent first. */
export const AUTOS = ["idle", "failed", "done", "running", "waiting"] as const;
export type Auto = (typeof AUTOS)[number];

/** Where a shown status comes from: manual, then rule or CLI, then auto. */
/** `launcher` is a launcher side tab's `n/m up` (D24). */
export type StatusSource = "manual" | "launcher" | "rule" | "cli" | "auto";

/** What a tab or side tab shows. `look` is absent when the auto status shows. */
export interface Shown {
  auto: Auto;
  /** A Claude session waits on the person: amber shows over any status. */
  waiting: boolean;
  source: StatusSource;
  look?: Look;
}

/** A status from the editor or Settings; global when `sideTabId` is null. */
export interface SavedStatus extends Look {
  id: Id;
  sideTabId: Id | null;
}

/** The global statuses a new database starts with; mirrors `DEFAULT_STATUSES` in status_store.rs. */
export const DEFAULT_STATUSES: readonly { label: string; accent: Accent }[] = [
  { label: "In progress", accent: "blue" },
  { label: "Review", accent: "magenta" },
  { label: "Testing", accent: "cyan" },
  { label: "Blocked", accent: "red" },
  { label: "Done", accent: "green" },
];

export type RuleKind = { kind: "exitCode"; pattern: string } | { kind: "claudeIdle"; minutes: number };

export type Rule = { id: Id; statusId: Id; look: Look; enabled: boolean } & RuleKind;

export interface StatusConfig {
  /** Global statuses in their order, then local ones. */
  statuses: SavedStatus[];
  rules: Rule[];
  /** Renamed or recolored built-ins; one left out keeps its default look. */
  builtins: Partial<Record<Auto, Look>>;
}

export interface Window {
  id: Id;
  sideTabs: SideTab[];
}

export interface Workspace {
  windows: Window[];
}

/** Where a tab dropped on a pane lands (task 8.32). */
export type DockSide = "left" | "right" | "up" | "down";

export type Command =
  | { type: "CreateSideTab"; windowId: Id; index?: number | null }
  | { type: "CreateTab"; sideTabId: Id }
  | { type: "CloseTab"; tabId: Id }
  | { type: "MoveTab"; tabId: Id; toSideTabId: Id; index: number }
  | { type: "MoveTabToNewSideTab"; tabId: Id; windowId: Id; index: number }
  /** Drop outside every window (D20): a new window with one side tab holding the tab. */
  | { type: "MoveTabToNewWindow"; tabId: Id }
  | { type: "RenameTab"; tabId: Id; title: string | null }
  /** A side tab's own name; null or blank goes back to its first tab's name (task 8.7). */
  | { type: "RenameSideTab"; sideTabId: Id; title: string | null }
  /** Closes a side tab with its tabs and its context (task 8.7). */
  | { type: "CloseSideTab"; sideTabId: Id }
  /** A new window holding the side tab, its tabs and its context (task 8.7). */
  | { type: "MoveSideTabToNewWindow"; sideTabId: Id }
  | { type: "SplitPane"; paneId: Id; dir: Dir }
  /** A tab dropped on a side of a pane (task 8.32): its panes join that tab, and it closes. */
  | { type: "DockTab"; tabId: Id; paneId: Id; side: DockSide }
  /** A pane dragged to the tab bar (task 8.32): a new tab at `index` in its side tab. */
  | { type: "PaneToTab"; paneId: Id; index: number }
  | { type: "OpenTranscript"; paneId: Id; session: string }
  /** A text pane editing `docId`: beside `beside` on the right, or in a new tab (D27). */
  | { type: "OpenText"; sideTabId: Id; docId: Id; beside?: Id | null }
  | { type: "ClosePane"; paneId: Id }
  | { type: "ResizeSplit"; splitId: Id; sizes: number[] }
  | { type: "ResumePane"; paneId: Id }
  | { type: "DismissResume"; paneId: Id }
  | { type: "SetStatus"; targetId: Id; status: ManualStatus | null }
  | { type: "SetIcon"; targetId: Id; icon: Icon | null }
  | { type: "SetAccent"; targetId: Id; accent: Neon }
  | { type: "RestyleStatus"; statusId: Id; status: ManualStatus | null };

export type Patch =
  /** An empty window; its side tab comes in the same batch. */
  | { type: "WindowCreated"; windowId: Id }
  /** A window that lost its last side tab while another remains (task 8.1). */
  | { type: "WindowClosed"; windowId: Id }
  | { type: "SideTabCreated"; windowId: Id; index: number; sideTab: SideTab }
  | { type: "SideTabClosed"; sideTabId: Id }
  | { type: "SideTabRenamed"; sideTabId: Id; title: string | null }
  | { type: "TabCreated"; sideTabId: Id; index: number; tab: Tab }
  | { type: "TabClosed"; tabId: Id }
  | { type: "TabMoved"; tabId: Id; toSideTabId: Id; index: number }
  | { type: "TabRenamed"; tabId: Id; title: string | null }
  | { type: "LayoutChanged"; tabId: Id; layout: Layout }
  | { type: "StatusSet"; id: Id; status: ManualStatus | null }
  | { type: "IconSet"; id: Id; icon: Icon | null }
  | { type: "AccentSet"; id: Id; accent: Neon };

export interface PatchBatch {
  rev: number;
  patches: Patch[];
}

export interface Snapshot {
  rev: number;
  workspace: Workspace;
}

export interface CommandError {
  code:
    | "UnknownWindow"
    | "UnknownSideTab"
    | "UnknownTab"
    | "IndexOutOfRange"
    | "UnknownPane"
    | "UnknownSplit"
    | "BadSizes"
    | "NothingToResume"
    | "PaneTooSmall"
    | "BadSession"
    | "BadStatus"
    | "BadIcon"
    | "BadAccent"
    | "DockOntoItself"
    | "OnlyPane";
  id: Id | number;
}

/** A Claude session's state in its pane (D34). */
export type AgentState = "idle" | "running" | "waiting" | "done";

export interface AgentView {
  session: string;
  state: AgentState;
  /** Claude's last reply from `Stop`, cut to a preview. */
  lastMessage: string | null;
}

/** One pane's agent changed; `agent: null` means the pane shows its shell again. */
export interface AgentUpdate {
  pane: Id;
  agent: AgentView | null;
}

/** One replacement a file tool asked for. A Write has an empty `old`. */
export interface ToolEdit {
  path: string;
  old: string;
  new: string;
}

/** A unified-diff hunk: `lines` start with " ", "-" or "+". */
export interface Hunk {
  oldStart: number;
  newStart: number;
  lines: string[];
}

/**
 * A transcript line as the core reads it (D11). One line can hold several entries: thinking and
 * tool blocks get the id `<uuid>#<block>`, the reply text keeps the plain uuid (task 8.4, D25).
 */
export type Entry = { id: string; offset: number; timestamp: string | null } & (
  | { kind: "user"; text: string }
  | { kind: "assistant"; text: string }
  | { kind: "thinking"; text: string }
  | { kind: "toolCall"; toolUseId: string; name: string; detail: string; edits: ToolEdit[] }
  | { kind: "toolResult"; toolUseId: string; text: string; isError: boolean; hunks: Hunk[] }
  | { kind: "toolDenied" }
  | { kind: "turnEnd" }
  | { kind: "interrupted" }
  | { kind: "compaction" }
  | { kind: "compactSummary"; text: string }
);

export interface TranscriptChunk {
  entries: Entry[];
  /** Where the next read starts. */
  offset: number;
  /** The file shrank or was replaced: `entries` start over from the top. */
  reset: boolean;
}

/**
 * A pin on a transcript entry, whole or a selection in it, or on rows of a terminal pane (D12).
 * Transcript pins carry `session` and `entryId`; terminal pins carry `paneId` and the rows.
 */
export interface Pin {
  id: Id;
  sideTabId: Id;
  originTabId: Id | null;
  session: string | null;
  entryId: string | null;
  paneId: Id | null;
  rowStart: number | null;
  rowEnd: number | null;
  exact: string;
  prefix: string | null;
  suffix: string | null;
  /** UTF-16 offsets of `exact` in the entry text, as the webview counts them; null for a whole entry. */
  start: number | null;
  end: number | null;
  createdAt: number;
}

export type NewPin = Omit<Pin, "id" | "createdAt">;

/** Where a variable lives (D21): SQLite, the daemon's memory, or the daemon and masked. */
export type VarMode = "saved" | "session" | "secret";
export const VAR_MODES: readonly VarMode[] = ["saved", "session", "secret"];

export interface Variable {
  id: Id;
  sideTabId: Id;
  name: string;
  /** Null for a secret: its value never reaches a window. */
  value: string | null;
  mode: VarMode;
  /** typed, pasted, extracted or auto. */
  source: string | null;
  originTabId: Id | null;
  /** When a TTL removes it, Unix ms. */
  expiresAt: number | null;
}

export interface NewVariable {
  name: string;
  value: string;
  mode: VarMode;
  ttlSecs?: number | null;
  source: string;
  originTabId?: Id | null;
}

/** Absent fields stay. A secret stays a secret; its TTL changes only with its value again. */
export interface VariableChange {
  value?: string;
  mode?: VarMode;
  ttlSecs?: number;
}

export type ItemKind = "link" | "text";

/** A link or a piece of text in a side tab's context. */
export interface Item {
  id: Id;
  sideTabId: Id;
  kind: ItemKind;
  value: string;
  source: string | null;
  originTabId: Id | null;
  createdAt: number;
}

/** A file Claude wrote or edited in a side tab, from a `PostToolUse` hook (D30). */
export interface TouchedFile {
  id: Id;
  sideTabId: Id;
  /** Absolute. */
  path: string;
  /** Relative to the session's folder when the file lies under it. */
  shown: string;
  change: "added" | "modified";
  originTabId: Id | null;
  updatedAt: number;
}

/** A variable both side tabs of a merge hold with different values (D5). */
export interface MergeClash {
  name: string;
  /** Null for a secret. */
  fromValue: string | null;
  fromMode: VarMode;
  intoValue: string | null;
  intoMode: VarMode;
  /** The name the source's variable takes when both stay, `branch_477` by default. */
  rename: string;
}

/** How the clashes of a merge were settled. */
export interface MergeResolution {
  /** Keep both, the source's renamed; otherwise the target's value wins. */
  keepBoth: boolean;
  /** Names typed over the suggested renames, by clashing name. */
  renames?: Record<string, string>;
}

export interface MergeDone {
  undoId: Id;
  /** Source variables kept under a new name: old, new. */
  renames: [string, string][];
}

/** A point in this window's coordinates (CSS pixels), during a drag from another window. */
export interface DragPoint {
  tabId: Id;
  x: number;
  y: number;
}

/**
 * Where a tab drag ended (task 8.1): over its own window, which drops it itself; over another
 * window, which got the drop; outside every window, where a new window holds the tab; or not
 * at all.
 */
export type DragEnd = { kind: "local" } | { kind: "remote" } | { kind: "newWindow" } | { kind: "cancelled" };

/** How long a merge offers undo, in ms (D5). */
export const MERGE_UNDO_MS = 10_000;

/** `branch_477` from `branch` and `PROJ-477`, else `branch_2`; never a name in `taken`. */
export function suggestRename(name: string, label: string | null | undefined, taken: ReadonlySet<string>): string {
  const digits = /(\d+)\s*$/.exec(label ?? "")?.[1];
  const base = digits ? `${name}_${digits}` : `${name}_2`;
  if (!taken.has(base)) return base;
  for (let n = 2; ; n++) if (!taken.has(`${base}_${n}`)) return `${base}_${n}`;
}

/** A library prompt at its current version (D14). */
export interface Prompt {
  id: Id;
  key: string;
  name: string;
  tags: string[];
  archived: boolean;
  version: number;
  body: string;
  /** The `{{names}}` the body uses, in order of first use. */
  variables: string[];
  lastUsedAt: number | null;
  useCount: number;
  createdAt: number;
  updatedAt: number;
}

export interface PromptVersion {
  version: number;
  body: string;
  createdAt: number;
}

export interface NewPrompt {
  key?: string | null;
  name: string;
  tags: string[];
  body: string;
}

/** The `{{name}}` placeholders of a body, each once, in order; the core's `variables_of`. */
export function variablesOf(body: string): string[] {
  const out: string[] = [];
  for (const m of body.matchAll(/\{\{([^{}]*)\}\}/g)) {
    const name = m[1]!.trim();
    if (isVariableName(name) && !out.includes(name)) out.push(name);
  }
  return out;
}

/** Names the shell or the loader acts on, as `is_shell_control` in crates/protocol lists them. */
export const SHELL_CONTROL_NAMES = [
  "PATH", "HOME", "SHELL", "USER", "LOGNAME", "IFS", "ENV", "BASH_ENV", "ZDOTDIR", "PROMPT_COMMAND",
  "PS0", "PS1", "PS2", "PS4", "PROMPT", "RPROMPT", "FPATH", "CDPATH", "TMPDIR", "LD_PRELOAD",
  "LD_LIBRARY_PATH", "XDG_CONFIG_HOME", "XDG_DATA_DIRS", "CLAUDE_ENV_FILE", "CLAUDE_CONFIG_DIR",
];

/**
 * `pr_api`, `API_TOKEN`: what `$NAME` and `{{name}}` accept. `TERMINESCU_*` is reserved, and the
 * names the shell acts on stay the shell's: `var env` runs before every prompt.
 */
export function isVariableName(name: string): boolean {
  return (
    /^[A-Za-z_][A-Za-z0-9_]{0,63}$/.test(name) &&
    !name.startsWith("TERMINESCU_") &&
    !SHELL_CONTROL_NAMES.includes(name) &&
    !/^(DYLD_|BASH_FUNC_|fish_)/.test(name)
  );
}

/** Settings › Notifications (workbench-ui "Attention signals"). */
export interface AttentionSettings {
  notify: boolean;
  chime: boolean;
  /** A command that ran `commandSecs` or longer posts a notification when it ends (task 8.40). */
  commandNotify: boolean;
  commandSecs: number;
}

/** The threshold's range in seconds. Same as `COMMAND_SECS` in attention.rs. */
export const COMMAND_SECS = [5, 3600] as const;

/** Longest TTL a variable default may hold: 30 days. Same as `MAX_TTL_SECS` in settings.rs. */
export const MAX_TTL_SECS = 30 * 86_400;

/** Settings › Variables and Shell (D25, D27). */
export interface AppSettings {
  /** The mode an extracted variable starts with. */
  varMode: VarMode;
  /** The TTL a session or secret variable starts with; null keeps it until its side tab ends. */
  varTtlSecs: number | null;
  /** `VISUAL` and `EDITOR` point at `terminescu edit --wait`. */
  editor: boolean;
  /** The zoom of every window, fonts and elements, in percent (task 8.8). */
  zoomPercent: number;
  /** The sidebar's width in CSS pixels, dragged on its right edge (task 8.23). */
  sidebarWidth: number;
  /** The sidebar shows only the rows' icons. */
  sidebarCollapsed: boolean;
  /** The terminal's font family; empty is the system mono font, SF Mono (task 8.36). */
  terminalFont: string;
  /** The terminal's font size in CSS pixels, before the zoom. */
  terminalFontSize: number;
  /** The terminal's line height in percent of the font size. */
  terminalLineHeight: number;
  /** Extra pixels between the terminal's characters. */
  terminalLetterSpacing: number;
  /** The terminal's font weight, 100 to 900 in steps of 100. */
  terminalFontWeight: number;
  /** The tab bar's text size in CSS pixels; a subtitle is one pixel smaller. */
  tabFontSize: number;
  /** The cursor's shape and blink (task 8.42); an unfocused terminal shows an outline. */
  terminalCursor: CursorStyle;
  terminalCursorBlink: boolean;
  /** Lines of history: open terminals take it at once, new shells' daemon copies too. */
  terminalScrollback: number;
  /** Which Option key sends Esc before the key, as Meta. */
  optionAsMeta: OptionMeta;
  /** The bell; visual and sound also bounce the dock icon in the background. */
  bell: Bell;
  /** Selecting text in a terminal copies it. */
  copyOnSelect: boolean;
  /** A long paste, a `sudo` line or a pipe into a shell asks first. */
  pasteWarning: boolean;
  /** Text too close to its background is drawn with more contrast. */
  raiseContrast: boolean;
  /** The terminal's colours (task 8.47); `imported` uses `terminalColors`. */
  terminalTheme: TerminalTheme;
  /** The colours last imported from Terminal.app or a file. */
  terminalColors: ImportedColors | null;
  /** 30 to 100; below 100 the desktop shows through the terminals' background. */
  terminalOpacity: number;
  /** Chords a focused terminal turns into shell editing keys (task 8.53); "" is off. */
  terminalKeys: TerminalKeys;
  /** The person's answer about the Claude Code hook (add-public-release D5). */
  hook: HookConsent;
  /** The first-run screen was seen to its end (D6). */
  firstRunDone: boolean;
  /** Tabs also show in the top bar; off, they show only in the sidebar (task 8.58). */
  topTabs: boolean;
}

export type HookConsent = "unasked" | "on" | "off";

/** Where the public project lives. Same as `public.rs` in the protocol crate. */
export const PUBLIC_REPO = "pantopix/terminescu";
export const MAKER = "PANTOPIX RO";
export const MAKER_URL = "https://pantopix.ro";
export const FEEDBACK_EMAIL = "salut@pantopix.com";
export const FEEDBACK_PREFIX = "[Terminescu]";

/** What About and the feedback routes say about the build and the Mac. */
export interface AppInfo {
  version: string;
  /** The commit the build came from, or `dev`. */
  build: string;
  macos: string;
}

export type FeedbackRoute = "email" | "issue";

/** As `TerminalKeys` in settings.rs. */
export interface TerminalKeys {
  wordLeft: string;
  wordRight: string;
  lineStart: string;
  lineEnd: string;
  deleteLine: string;
  newline: string;
}

/** Each terminal key: its label in Settings › Keys and the bytes the shell reads. */
export const TERMINAL_KEYS: readonly { id: keyof TerminalKeys; label: string; text: string }[] = [
  { id: "wordLeft", label: "Back one word", text: "\x1bb" },
  { id: "wordRight", label: "Forward one word", text: "\x1bf" },
  { id: "lineStart", label: "Start of line", text: "\x01" },
  { id: "lineEnd", label: "End of line", text: "\x05" },
  { id: "deleteLine", label: "Delete to line start", text: "\x15" },
  { id: "newline", label: "New line without running (Claude)", text: "\x1b\r" },
];

export type TerminalTheme = "terminescu" | "basic" | "solarizedDark" | "solarizedLight" | "imported";
export const TERMINAL_THEMES: readonly TerminalTheme[] = ["terminescu", "basic", "solarizedDark", "solarizedLight", "imported"];
/** The terminal opacity's range in percent. Same as `OPACITIES` in settings.rs. */
export const OPACITIES = [30, 100] as const;

/** Colours from an imported profile, as `term_import::Colors`: `#rrggbb`, 16 ANSI entries. */
export interface ImportedColors {
  foreground: string;
  background: string;
  cursor: string | null;
  selection: string | null;
  bold: string | null;
  ansi: string[];
}

/** A Terminal.app profile or iTerm2 preset as `term_import::Profile` reads it. */
export interface TerminalProfile {
  name: string;
  colors: ImportedColors | null;
  /** "" is the system mono font. */
  fontFamily: string | null;
  fontSize: number | null;
  backgroundOpacity: number | null;
  cursor: CursorStyle | null;
  cursorBlink: boolean | null;
  optionAsMeta: boolean | null;
}

export type CursorStyle = "bar" | "block" | "underline";
export const CURSOR_STYLES: readonly CursorStyle[] = ["bar", "block", "underline"];
export type OptionMeta = "both" | "left" | "right" | "off";
export const OPTION_METAS: readonly OptionMeta[] = ["both", "left", "right", "off"];
export type Bell = "off" | "visual" | "sound";
export const BELLS: readonly Bell[] = ["off", "visual", "sound"];
/** Lines of history a terminal keeps. Same as `SCROLLBACKS` in settings.rs. */
export const SCROLLBACKS = [1_000, 50_000] as const;

/** Ranges Settings › Terminal allows. Same as settings.rs. */
export const FONT_SIZES = [9, 24] as const;
export const LINE_HEIGHTS = [100, 200] as const;
export const LETTER_SPACINGS = [-2, 4] as const;
export const TAB_FONT_SIZES = [9, 14] as const;
export const FONT_FAMILY_MAX = 64;
/** A font name keeps to characters a CSS font list takes unquoted. Same check as settings.rs. */
export const FONT_NAME = /^[\p{L}\p{N} ._-]*$/u;

/** The sidebar's default width and the range a drag may set. Same as settings.rs. */
export const SIDEBAR_WIDTH = 272;
export const SIDEBAR_MIN = 180;
export const SIDEBAR_MAX = 480;

export const DEFAULT_SETTINGS: AppSettings = {
  varMode: "saved",
  varTtlSecs: null,
  editor: false,
  zoomPercent: 100,
  sidebarWidth: SIDEBAR_WIDTH,
  sidebarCollapsed: false,
  terminalFont: "",
  terminalFontSize: 11,
  terminalLineHeight: 100,
  terminalLetterSpacing: 0,
  terminalFontWeight: 400,
  tabFontSize: 12,
  terminalCursor: "bar",
  terminalCursorBlink: true,
  terminalScrollback: 10_000,
  optionAsMeta: "both",
  bell: "off",
  copyOnSelect: false,
  pasteWarning: true,
  raiseContrast: false,
  terminalTheme: "terminescu",
  terminalColors: null,
  terminalOpacity: 100,
  terminalKeys: {
    wordLeft: "Alt+Left",
    wordRight: "Alt+Right",
    lineStart: "Cmd+Left",
    lineEnd: "Cmd+Right",
    deleteLine: "Cmd+Backspace",
    newline: "",
  },
  hook: "unasked",
  firstRunDone: false,
  topTabs: false,
};

/** The steps `⌘+` and `⌘-` walk, in percent. Same as `ZOOM_STEPS` in settings.rs. */
export const ZOOM_STEPS: readonly number[] = [50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200];

/** The next zoom step in or out from `percent`, which need not be a step; the ends stay put. */
export function stepZoom(percent: number, dir: 1 | -1): number {
  if (dir > 0) return ZOOM_STEPS.find((z) => z > percent) ?? ZOOM_STEPS.at(-1)!;
  return [...ZOOM_STEPS].reverse().find((z) => z < percent) ?? ZOOM_STEPS[0]!;
}

/** Where a saved command runs (D24). */
export type CommandTarget = "focused" | "newTab" | "newSideTab";
export const COMMAND_TARGETS: readonly CommandTarget[] = ["focused", "newTab", "newSideTab"];

/** A saved shell line with `{{variables}}`, for one side tab or global (`sideTabId` null). */
export interface SavedCommand {
  id: Id;
  sideTabId: Id | null;
  name: string;
  body: string;
  target: CommandTarget;
  launcher: boolean;
  /** The tab a launcher command runs in, while it exists. */
  tabId: Id | null;
  position: number;
  createdAt: number;
}

export interface NewCommand {
  sideTabId: Id | null;
  name: string;
  body: string;
  target: CommandTarget;
  launcher: boolean;
}

export interface RunRequest {
  windowId: Id;
  sideTabId: Id;
  id: Id;
  /** Overrides the saved target. */
  target?: CommandTarget;
  /** The focused pane, for the "focused" target. */
  paneId?: Id | null;
  /** Run sends Enter; insert does not. */
  run: boolean;
  /** Values typed for missing names. */
  extra?: Record<string, string>;
}

/** Ask for `missing` names; refuse Run on values with line breaks; or the pane that got it. */
export type RunOutcome =
  | { kind: "missing"; names: string[] }
  | { kind: "unsafe"; names: string[] }
  | { kind: "done"; pane: Id | null };

/** Where a text pane's document comes from and saves to (D27, D28). */
export type DocSource =
  | { kind: "file"; path: string }
  | { kind: "untitled" }
  | { kind: "variable"; sideTabId: Id; name: string }
  | { kind: "prompt"; promptId: Id };

/**
 * What a text pane shows, as crates/core/src/documents.rs sends it. `text` is absent when the
 * editor already has it.
 */
export interface DocView {
  id: Id;
  source: DocSource;
  title: string;
  text?: string;
  dirty: boolean;
  /** The file changed on disk under unsaved edits: Reload or Keep mine. */
  conflict: boolean;
  readOnly: boolean;
  bom: boolean;
  /** Not on disk yet; saving creates it. */
  missing: boolean;
  /** A session or secret variable: never saved to a file. */
  sessionOnly: boolean;
  /** `terminescu edit --wait` waits on it. */
  request: boolean;
  note?: string;
  /** Bumps when the core replaced the text (a reload). */
  rev: number;
}

/** Files for `⌘O`, relative to `root`. */
export interface FileList {
  root: string;
  files: string[];
  /** More files exist than the list holds. */
  truncated: boolean;
}

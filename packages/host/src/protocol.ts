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
}

export interface SideTab {
  id: Id;
  title: string | null;
  tabs: Tab[];
  status?: ManualStatus;
}

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

export type Command =
  | { type: "CreateSideTab"; windowId: Id; index?: number | null }
  | { type: "CreateTab"; sideTabId: Id }
  | { type: "CloseTab"; tabId: Id }
  | { type: "MoveTab"; tabId: Id; toSideTabId: Id; index: number }
  | { type: "MoveTabToNewSideTab"; tabId: Id; windowId: Id; index: number }
  | { type: "RenameTab"; tabId: Id; title: string | null }
  | { type: "SplitPane"; paneId: Id; dir: Dir }
  | { type: "OpenTranscript"; paneId: Id; session: string }
  /** A text pane editing `docId`: beside `beside` on the right, or in a new tab (D27). */
  | { type: "OpenText"; sideTabId: Id; docId: Id; beside?: Id | null }
  | { type: "ClosePane"; paneId: Id }
  | { type: "ResizeSplit"; splitId: Id; sizes: number[] }
  | { type: "ResumePane"; paneId: Id }
  | { type: "DismissResume"; paneId: Id }
  | { type: "SetStatus"; targetId: Id; status: ManualStatus | null }
  | { type: "RestyleStatus"; statusId: Id; status: ManualStatus | null };

export type Patch =
  | { type: "SideTabCreated"; windowId: Id; index: number; sideTab: SideTab }
  | { type: "SideTabClosed"; sideTabId: Id }
  | { type: "TabCreated"; sideTabId: Id; index: number; tab: Tab }
  | { type: "TabClosed"; tabId: Id }
  | { type: "TabMoved"; tabId: Id; toSideTabId: Id; index: number }
  | { type: "TabRenamed"; tabId: Id; title: string | null }
  | { type: "LayoutChanged"; tabId: Id; layout: Layout }
  | { type: "StatusSet"; id: Id; status: ManualStatus | null };

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
    | "BadStatus";
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

/** `pr_api`, `API_TOKEN`: what `$NAME` and `{{name}}` accept. `TERMINESCU_*` is reserved. */
export function isVariableName(name: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]{0,63}$/.test(name) && !name.startsWith("TERMINESCU_");
}

/** Settings › Notifications (workbench-ui "Attention signals"). */
export interface AttentionSettings {
  notify: boolean;
  chime: boolean;
}

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
}

export const DEFAULT_SETTINGS: AppSettings = { varMode: "saved", varTtlSecs: null, editor: false };

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

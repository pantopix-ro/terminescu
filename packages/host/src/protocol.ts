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
export type StatusSource = "manual" | "rule" | "cli" | "auto";

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

/** A transcript line as the core reads it (D11). v1 shows user and assistant text (D25). */
export type Entry = { id: string; offset: number; timestamp: string | null } & (
  | { kind: "user"; text: string }
  | { kind: "assistant"; text: string }
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

/** A pin on a transcript entry, whole or a selection in it (D12). */
export interface Pin {
  id: Id;
  sideTabId: Id;
  session: string;
  entryId: string;
  exact: string;
  prefix: string | null;
  suffix: string | null;
  /** UTF-16 offsets of `exact` in the entry text, as the webview counts them; null for a whole entry. */
  start: number | null;
  end: number | null;
  createdAt: number;
}

export type NewPin = Omit<Pin, "id" | "createdAt">;

export interface Variable {
  id: Id;
  sideTabId: Id;
  name: string;
  value: string;
  source: string | null;
}

import type {
  AgentUpdate,
  Auto,
  Command,
  Id,
  Item,
  ItemKind,
  Look,
  NewVariable,
  NewPin,
  PatchBatch,
  Pin,
  Prompt,
  PromptVersion,
  NewPrompt,
  Rule,
  RuleKind,
  SavedStatus,
  Shown,
  Snapshot,
  StatusConfig,
  TranscriptChunk,
  Variable,
  VariableChange,
} from "./protocol";

/** One window's view of a pane (D33). */
export interface PaneView {
  /** Keys and pastes. `binary` marks xterm.js `onBinary` data: one byte per character. */
  input(data: string, binary?: boolean): void;
  resize(cols: number, rows: number): void;
  /** Reports bytes xterm.js has written, so the core keeps sending. */
  ack(bytes: number): void;
  close(): void;
}

/**
 * How a window talks to the core (D6). v1 runs it over Tauri IPC; the remote view (D22) will
 * run the same interface over WebSocket.
 */
export interface Host {
  snapshot(): Promise<Snapshot>;
  /** Sends a command; resolves with the rev of the batch it produced. */
  send(command: Command): Promise<number>;
  /** Resolves once the listener is registered, with a function that removes it. */
  onPatches(listener: (batch: PatchBatch) => void): Promise<() => void>;
  /**
   * Shows a pane at the given size. Output arrives as raw bytes, starting with a full redraw;
   * the same happens again whenever the core resyncs the view.
   */
  openPane(paneId: Id, cols: number, rows: number, onOutput: (bytes: Uint8Array) => void): Promise<PaneView>;
  /**
   * App actions from `⌘` chords (D17): the app menu in Tauri, key events on the mock. Resolves
   * once the listener is registered, with a function that removes it.
   */
  onAction(listener: (action: string) => void): Promise<() => void>;
  /** Every pane's Claude session right now (D34). */
  agents(): Promise<AgentUpdate[]>;
  /** Resolves once the listener is registered, with a function that removes it. */
  onAgent(listener: (update: AgentUpdate) => void): Promise<() => void>;
  /** Entries a session's transcript gained since `offset`; start at 0 (task 4.4). */
  readTranscript(session: string, offset: number): Promise<TranscriptChunk>;
  pins(sideTabId: Id): Promise<Pin[]>;
  addPin(pin: NewPin): Promise<Pin>;
  removePin(id: Id): Promise<void>;
  /** Saved variables, then session and secret ones; a secret comes without its value (D21). */
  variables(sideTabId: Id): Promise<Variable[]>;
  /** Adds a variable; a name in use is refused unless `replace` (D13, D21). */
  addVariable(sideTabId: Id, variable: NewVariable, replace?: boolean): Promise<Variable>;
  changeVariable(sideTabId: Id, name: string, change: VariableChange): Promise<Variable>;
  removeVariable(sideTabId: Id, name: string): Promise<void>;
  /** Links and text of a side tab's context. */
  items(sideTabId: Id): Promise<Item[]>;
  /** The same link twice is kept once. */
  addItem(sideTabId: Id, kind: ItemKind, value: string, source: string, originTabId?: Id | null): Promise<Item>;
  removeItem(id: Id): Promise<void>;
  /**
   * The prompt library (D14). Every word must match: `#tag` a whole tag, any other word the
   * name, a tag or the body. Archived prompts come only with `archived`. Last used first.
   */
  searchPrompts(query: string, archived?: boolean): Promise<Prompt[]>;
  createPrompt(prompt: NewPrompt): Promise<Prompt>;
  /** A new version when the body changed. */
  savePrompt(id: Id, body: string): Promise<Prompt>;
  /** Renames and retags without a version. */
  updatePrompt(id: Id, name: string, tags: string[]): Promise<Prompt>;
  /** Newest first. */
  promptVersions(id: Id): Promise<PromptVersion[]>;
  /** The old body becomes a new version. */
  restorePrompt(id: Id, version: number): Promise<Prompt>;
  duplicatePrompt(id: Id): Promise<Prompt>;
  archivePrompt(id: Id, archived: boolean): Promise<Prompt>;
  /** Counts an insert: last used and use count. */
  markPromptUsed(id: Id): Promise<void>;
  /** A prompt changed in any window. Resolves with a function that removes the listener. */
  onPrompts(listener: () => void): Promise<() => void>;
  /** A side tab's context changed. Resolves with a function that removes the listener. */
  onContext(listener: (sideTabId: Id) => void): Promise<() => void>;
  /** Every tab's and side tab's shown status, by id (D15). */
  statuses(): Promise<Record<Id, Shown>>;
  /** The whole map again whenever a status changes. Resolves with a function that removes the listener. */
  onStatuses(listener: (statuses: Record<Id, Shown>) => void): Promise<() => void>;
  /** Saved statuses, rules and built-in looks, for the editor and Settings › Statuses. */
  statusConfig(): Promise<StatusConfig>;
  /** The config changed in some window. Resolves with a function that removes the listener. */
  onStatusConfig(listener: () => void): Promise<() => void>;
  /** Saves a status, local to a side tab or global (`null`). */
  saveStatus(sideTabId: Id | null, look: Look): Promise<SavedStatus>;
  /** Edits a saved status; every tab using it takes the new look. */
  updateStatus(id: Id, look: Look): Promise<SavedStatus>;
  /** Deletes a saved status and its rules; tabs using it go back to auto. */
  deleteStatus(id: Id): Promise<void>;
  makeGlobal(id: Id): Promise<SavedStatus>;
  /** Puts the global statuses in this order. */
  reorderStatuses(ids: Id[]): Promise<void>;
  addRule(statusId: Id, kind: RuleKind): Promise<Rule>;
  toggleRule(id: Id, enabled: boolean): Promise<void>;
  deleteRule(id: Id): Promise<void>;
  /** Renames or recolors a built-in status; `null` restores its default. */
  setBuiltin(auto: Auto, look: Look | null): Promise<void>;
  /** Opens an http(s) link in the default browser. */
  openUrl(url: string): Promise<void>;
}

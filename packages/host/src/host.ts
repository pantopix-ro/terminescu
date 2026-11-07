import type { KeyBindings } from "./keys";
import type {
  TerminalProfile,
  AgentUpdate,
  PaneLabel,
  DragEnd,
  DragPoint,
  AttentionSettings,
  AppSettings,
  NewCommand,
  RunOutcome,
  RunRequest,
  SavedCommand,
  Auto,
  Command,
  DocView,
  FileList,
  Id,
  Item,
  TouchedFile,
  MergeClash,
  MergeDone,
  MergeResolution,
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
  AppInfo,
  FeedbackRoute,
} from "./protocol";

/** One window's view of a pane (D33). */
export interface PaneView {
  /** Keys and pastes. `binary` marks xterm.js `onBinary` data: one byte per character. */
  input(data: string, binary?: boolean): void;
  resize(cols: number, rows: number): void;
  /** Reports bytes xterm.js has written, so the core keeps sending. */
  ack(bytes: number): void;
  /**
   * The terminal cleared its history (`keepScreen`) or cleared to its prompt line: the daemon
   * does the same to its copy, so a reattach does not bring the lines back (task 8.44).
   */
  clear(keepScreen: boolean): void;
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
  /**
   * `⌘⇧A`: the core picks the pane that has waited longest, raises its window and tells that
   * window through `onRevealPane`. Resolves with the pane, or null when nothing waits.
   */
  jumpWaiting(): Promise<Id | null>;
  /** This window must show and focus a pane. Resolves with a function that removes the listener. */
  onRevealPane(listener: (paneId: Id) => void): Promise<() => void>;
  /** This window plays the chime: a session started waiting and the chime is on. */
  onChime(listener: () => void): Promise<() => void>;
  /** Settings › Notifications: notification on, chime off by default. */
  attentionSettings(): Promise<AttentionSettings>;
  setAttentionSettings(settings: AttentionSettings): Promise<void>;
  /** A side tab's own commands, then the global ones; `null` for the global ones only (D24). */
  commands(sideTabId: Id | null): Promise<SavedCommand[]>;
  /** Every command, global ones first (Settings › Commands). */
  allCommands(): Promise<SavedCommand[]>;
  settings(): Promise<AppSettings>;
  setSettings(settings: AppSettings): Promise<AppSettings>;
  /** Settings changed in any window. */
  onSettings(listener: () => void): Promise<() => void>;
  /** Chords changed in Settings › Keys, by action id (D17). */
  keyBindings(): Promise<KeyBindings>;
  /** Saves them all at once; a clash or a chord without ⌘ is refused. */
  setKeyBindings(bindings: KeyBindings): Promise<KeyBindings>;
  onKeyBindings(listener: () => void): Promise<() => void>;
  /** While on, the menu holds no action chords, so a recorder sees every ⌘ key. */
  recordKeys(on: boolean): Promise<void>;
  createCommand(command: NewCommand): Promise<SavedCommand>;
  /** New name, body, target and launcher flag; the scope stays. */
  updateCommand(id: Id, command: NewCommand): Promise<SavedCommand>;
  deleteCommand(id: Id): Promise<void>;
  /** Fills and types a command: Run with Enter, Insert without (D24). */
  runSavedCommand(request: RunRequest): Promise<RunOutcome>;
  /** Runs a side tab's launcher commands that are down again: `id`, or all of them. */
  restartLauncher(sideTabId: Id, id?: Id): Promise<void>;
  /** Commands changed in any window. Resolves with a function that removes the listener. */
  onCommands(listener: () => void): Promise<() => void>;
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
  /** Files Claude wrote or edited in a side tab, last touched first (D30). */
  touchedFiles(sideTabId: Id): Promise<TouchedFile[]>;
  /** Drops a Files entry; the file stays on disk. */
  forgetFile(id: Id): Promise<void>;
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
  /** Every terminal pane's live label, by pane id (task 8.30). */
  labels(): Promise<Record<Id, PaneLabel>>;
  /** The whole map again whenever a label changes. Resolves with a function that removes the listener. */
  onLabels(listener: (labels: Record<Id, PaneLabel>) => void): Promise<() => void>;
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
  /** The version, build and macOS version, for About and feedback (add-public-release). */
  appInfo(): Promise<AppInfo>;
  /** Opens the feedback email or a new GitHub issue; `includeLog` shows app.log in Finder. */
  feedbackOpen(route: FeedbackRoute, includeLog: boolean): Promise<void>;
  /** Removes the hook, the daemon and the copied helpers, and the data when asked (D9). */
  uninstallHelpers(deleteData: boolean): Promise<void>;
  /** Quits the app. */
  appQuit(): Promise<void>;
  /**
   * A path link's menu (task 8.49): `reveal` shows it in Finder, else its default app opens it.
   * Resolved against the pane's folder; programs and scripts are refused.
   */
  openPathWith(paneId: Id, path: string, reveal: boolean): Promise<void>;
  /** Bounces the dock icon once, for a terminal's bell while the app is in the background. */
  bounceDock(): Promise<void>;
  /** Settings › Terminal (task 8.47): Terminal.app's default profile. */
  importTerminalApp(): Promise<TerminalProfile>;
  /** A `.terminal` profile or an `.itermcolors` preset. */
  importTerminalFile(path: string): Promise<TerminalProfile>;
  /** `⌥⌘T`: an Untitled text pane beside `beside`, or in a new tab of the side tab (D27). */
  newText(sideTabId: Id, beside?: Id | null): Promise<DocView>;
  /**
   * Opens a file by absolute path in a text pane. A file already open comes back without a new
   * pane; the window focuses the pane that shows it.
   */
  openFile(sideTabId: Id, path: string, beside?: Id | null): Promise<DocView>;
  /**
   * `⌘`-click on a path in terminal output: `path` may be relative to the pane's cwd or start
   * with `~`. Only an existing file opens.
   */
  openPath(sideTabId: Id, paneId: Id, path: string, beside?: Id | null): Promise<DocView>;
  /** A prompt in a text tab; `⌘S` saves a new version (D14). An open one comes back as it is. */
  openPrompt(sideTabId: Id, promptId: Id, beside?: Id | null): Promise<DocView>;
  /** A saved or session variable in a text tab; `⌘S` changes its value. Secrets never open. */
  openVariable(sideTabId: Id, name: string, beside?: Id | null): Promise<DocView>;
  /** `⌘↵`: the `terminescu edit --wait` call on this document returns. Save and close around it. */
  returnEdit(docId: Id): Promise<void>;
  /**
   * The variables that clash if side tab `fromId` merges into `intoId` (D5). `label` is the
   * source's name as the sidebar shows it, for the suggested renames.
   */
  mergePreview(fromId: Id, intoId: Id, label: string | null): Promise<MergeClash[]>;
  /** Drops side tab `fromId` onto `intoId`: tabs move, lists concatenate, variables union. */
  mergeSideTabs(fromId: Id, intoId: Id, label: string | null, resolution: MergeResolution): Promise<MergeDone>;
  /** Undoes a merge within `MERGE_UNDO_MS`. */
  undoMerge(undoId: Id): Promise<void>;
  /**
   * A tab drag started here; the core tracks the cursor across windows until `dragEnd`.
   * Resolves to the drag's token.
   */
  dragBegin(tabId: Id, title: string): Promise<number>;
  /** The drag `token` ended: `drop` false for a cancel. Says where it landed. */
  dragEnd(token: number, drop: boolean): Promise<DragEnd>;
  /** A drag from another window is over this one at a point, or left it (`null`). */
  onDragHover(listener: (point: DragPoint | null) => void): Promise<() => void>;
  /** A tab from another window was dropped here at a point. */
  onDragDrop(listener: (point: DragPoint) => void): Promise<() => void>;
  /** Pastes `text` into a terminal pane, bracketed when it has line breaks, with no Enter (D13). */
  sendText(paneId: Id, text: string): Promise<void>;
  /**
   * Saves a pasted image next to the document's file, or with an Untitled buffer's first save,
   * and resolves to the file name to link (D29). `ext` is png, jpg, gif or webp.
   */
  pasteImage(docId: Id, bytes: Uint8Array, ext: string): Promise<string>;
  /**
   * Files dropped from Finder on the window, as absolute paths, with the drop point in CSS
   * pixels of the page when known (task 8.43).
   */
  onFileDrop(listener: (paths: string[], at?: { x: number; y: number }) => void): Promise<() => void>;
  /** A document with its text. */
  document(docId: Id): Promise<DocView>;
  /** The editor's whole text, after a pause in typing (D28). */
  /**
   * The editor's text, typed over revision `rev`. When the core has loaded newer text since,
   * the text still wins but the document reports a conflict (the changed-on-disk bar).
   */
  editDocument(docId: Id, text: string, rev?: number): Promise<DocView>;
  /** `⌘S`; `path` saves under a new name. Untitled without a path fails: ask for one first. */
  saveDocument(docId: Id, path?: string | null): Promise<DocView>;
  /** Changed on disk: take the disk's text. */
  reloadDocument(docId: Id): Promise<DocView>;
  /** Changed on disk: keep the buffer. */
  keepMine(docId: Id): Promise<DocView>;
  /** A document changed outside the editor: reloaded, changed on disk or gone missing. */
  onDocument(listener: (doc: DocView) => void): Promise<() => void>;
  /** Files under a pane's cwd for `⌘O`, git-tracked ones when it is a repository. */
  listFiles(paneId: Id | null): Promise<FileList>;
  /** The macOS Open dialog; null when cancelled. */
  chooseFile(defaultDir?: string): Promise<string | null>;
  /** The macOS Save dialog; null when cancelled. */
  chooseSavePath(defaultPath?: string): Promise<string | null>;
}

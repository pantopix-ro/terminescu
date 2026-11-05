import type { AgentUpdate, Command, Id, NewPin, PatchBatch, Pin, Snapshot, TranscriptChunk, Variable } from "./protocol";

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
  variables(sideTabId: Id): Promise<Variable[]>;
  /** Saves extracted text as a variable; a name in use is refused (D13). */
  extractVariable(sideTabId: Id, name: string, value: string): Promise<Variable>;
  /** A side tab's pins or variables changed. Resolves with a function that removes the listener. */
  onContext(listener: (sideTabId: Id) => void): Promise<() => void>;
  /** Opens an http(s) link in the default browser. */
  openUrl(url: string): Promise<void>;
}

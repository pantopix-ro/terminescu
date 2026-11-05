import type { Command, Id, PatchBatch, Snapshot } from "./protocol";

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
  /** Opens an http(s) link in the default browser. */
  openUrl(url: string): Promise<void>;
}

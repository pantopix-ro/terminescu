// Byte-exact text for the editor (D29): CodeMirror keeps one line separator per document, so a
// file keeps the line endings it came with.

import { EditorState, type Extension } from "@codemirror/state";

export type Eol = "LF" | "CRLF" | "Mixed";

/** How a text ends its lines: every `\n` after a `\r` is CRLF, none is LF, both is Mixed. */
export function eolOf(text: string): Eol {
  const lf = (text.match(/\n/g) ?? []).length;
  const crlf = (text.match(/\r\n/g) ?? []).length;
  if (crlf === 0) return "LF";
  return crlf === lf ? "CRLF" : "Mixed";
}

/**
 * The separator CodeMirror splits and joins lines on. A Mixed file splits on `\n` alone, so its
 * `\r` bytes stay in the text as visible characters and save as they were.
 */
export function separatorOf(text: string): "\r\n" | "\n" {
  return eolOf(text) === "CRLF" ? "\r\n" : "\n";
}

/** An editor state over `text` that gives back the same bytes from [`textOf`]. */
export function exactState(text: string, extensions: Extension = []): EditorState {
  return EditorState.create(exactConfig(text, extensions));
}

/** The config {@link exactState} builds its state from, for an editor that builds its own. */
export function exactConfig(text: string, extensions: Extension = []): { doc: string; extensions: Extension } {
  return { doc: text, extensions: [EditorState.lineSeparator.of(separatorOf(text)), extensions] };
}

/**
 * Text from outside (a paste, a drop, an insert) with its line breaks turned into the
 * document's own: CodeMirror splits only on the separator, so a `\n` pasted into a CRLF file
 * would stay a bare LF byte in the middle of a line. A Mixed file takes text as it comes.
 */
export function fitBreaks(text: string, state: EditorState): string {
  const doc = textOf(state);
  const eol = doc.includes("\n") ? eolOf(doc) : state.lineBreak === "\r\n" ? "CRLF" : "LF";
  if (eol === "CRLF") return text.replace(/\r?\n/g, "\r\n");
  if (eol === "LF") return text.replace(/\r\n/g, "\n");
  return text;
}

/** The document joined with its own separator, as it goes back to the core. */
export function textOf(state: EditorState): string {
  return state.sliceDoc();
}

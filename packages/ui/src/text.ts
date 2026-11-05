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
  return EditorState.create({
    doc: text,
    extensions: [EditorState.lineSeparator.of(separatorOf(text)), extensions],
  });
}

/** The document joined with its own separator, as it goes back to the core. */
export function textOf(state: EditorState): string {
  return state.sliceDoc();
}

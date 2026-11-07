/** The modifier and key fields of a key event that the terminal's own chords read. */
export interface KeyLike {
  key: string;
  metaKey: boolean;
  altKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
}

/**
 * Text a terminal sends for the macOS editing chords Terminal.app and iTerm2 users expect, or
 * undefined for any other key. `⌥←` and `⌥→` move a word (Esc b, Esc f) as Terminal.app does,
 * where xterm.js would send `Esc [1;3D`, which zsh does not bind. `⌘←`, `⌘→` and `⌘⌫` go to the
 * line's start, its end and delete to its start (^A, ^E, ^U), as iTerm2's natural text editing
 * does. None of these is a menu chord.
 */
export function terminalKeyText(e: KeyLike): string | undefined {
  if (e.ctrlKey || e.shiftKey) return undefined;
  if (e.altKey && !e.metaKey) {
    if (e.key === "ArrowLeft") return "\x1bb";
    if (e.key === "ArrowRight") return "\x1bf";
    return undefined;
  }
  if (e.metaKey && !e.altKey) {
    if (e.key === "ArrowLeft") return "\x01";
    if (e.key === "ArrowRight") return "\x05";
    if (e.key === "Backspace") return "\x15";
  }
  return undefined;
}

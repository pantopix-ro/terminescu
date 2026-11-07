import { DEFAULT_SETTINGS, parseAccel, TERMINAL_KEYS, type TerminalKeys } from "@terminescu/host";

/** The modifier and key fields of a key event that the terminal's own chords read. */
export interface KeyLike {
  key: string;
  code?: string;
  metaKey: boolean;
  altKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
}

/** The key a chord names, from the physical key for letters and digits so ⌥ does not change it. */
function keyOf(e: KeyLike): string {
  if (e.code && /^Key[A-Z]$/.test(e.code)) return e.code.slice(3).toLowerCase();
  if (e.code && /^Digit[0-9]$/.test(e.code)) return e.code.slice(5);
  return e.key.length === 1 ? e.key.toLowerCase() : e.key;
}

/**
 * Text a terminal sends for the editing chords in Settings › Keys (task 8.53), or undefined for
 * any other key. By default `⌥←` and `⌥→` move a word (Esc b, Esc f) as Terminal.app does, where
 * xterm.js would send `Esc [1;3D`, which zsh does not bind; `⌘←`, `⌘→` and `⌘⌫` go to the line's
 * start, its end and delete to its start (^A, ^E, ^U), as iTerm2's natural text editing does.
 * The new line chord (Esc CR, as `⌥↵` in Claude) is off until the person sets one.
 */
export function terminalKeyText(e: KeyLike, keys: TerminalKeys = DEFAULT_SETTINGS.terminalKeys): string | undefined {
  const key = keyOf(e);
  for (const k of TERMINAL_KEYS) {
    const accel = keys[k.id];
    if (!accel) continue;
    const c = parseAccel(accel);
    if (c.key.toLowerCase() === key.toLowerCase() && c.meta === e.metaKey && c.alt === e.altKey && c.ctrl === e.ctrlKey && c.shift === e.shiftKey)
      return k.text;
  }
  return undefined;
}

// Chords for app actions (D17), read from actions.json, the same file the app menu reads.

import actionsFile from "./actions.json";

export interface ActionDef {
  id: string;
  label: string;
  menu: string;
  accel: string;
}

export const ACTIONS: readonly ActionDef[] = actionsFile.actions;

const KEY_NAMES: Record<string, string> = {
  backslash: "\\",
  comma: ",",
  period: ".",
  slash: "/",
  semicolon: ";",
  quote: "'",
  bracketleft: "[",
  bracketright: "]",
  minus: "-",
  equal: "=",
  backquote: "`",
  left: "ArrowLeft",
  right: "ArrowRight",
  up: "ArrowUp",
  down: "ArrowDown",
};

interface Chord {
  meta: boolean;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  key: string;
}

export function parseAccel(accel: string): Chord {
  const parts = accel.split("+").map((p) => p.trim().toLowerCase());
  const key = parts.at(-1)!;
  return {
    meta: parts.includes("cmd"),
    ctrl: parts.includes("ctrl"),
    alt: parts.includes("alt"),
    shift: parts.includes("shift"),
    key: KEY_NAMES[key] ?? key,
  };
}

/** Physical keys whose accelerator name is a word, as Tauri spells it. */
const CODE_NAMES: Record<string, string> = {
  Backslash: "Backslash",
  Comma: "Comma",
  Period: "Period",
  Slash: "Slash",
  Semicolon: "Semicolon",
  Quote: "Quote",
  BracketLeft: "BracketLeft",
  BracketRight: "BracketRight",
  Minus: "Minus",
  Equal: "Equal",
  Backquote: "Backquote",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  ArrowUp: "Up",
  ArrowDown: "Down",
};

/** The key a chord ends on, from the physical key so ⌥ and ⇧ don't change it. */
function keyOf(e: KeyboardEvent): string {
  if (/^Key[A-Z]$/.test(e.code)) return e.code.slice(3).toLowerCase();
  if (/^Digit[0-9]$/.test(e.code)) return e.code.slice(5);
  if (/^F([1-9]|1[0-9])$/.test(e.code)) return e.code.toLowerCase();
  const named = CODE_NAMES[e.code];
  if (named) return KEY_NAMES[named.toLowerCase()] ?? named;
  return e.key.length === 1 ? e.key.toLowerCase() : e.key;
}

/** The action a key event triggers, if any. */
export function actionFor(e: KeyboardEvent, actions: readonly ActionDef[] = ACTIONS): string | undefined {
  const key = keyOf(e);
  return actions.find((a) => {
    const c = parseAccel(a.accel);
    return c.key === key && c.meta === e.metaKey && c.ctrl === e.ctrlKey && c.alt === e.altKey && c.shift === e.shiftKey;
  })?.id;
}

/** How a chord reads in the UI: ⌃⌥⇧⌘ then the key. */
export function chordLabel(accel: string): string {
  const c = parseAccel(accel);
  const key = { "\\": "\\", ArrowLeft: "←", ArrowRight: "→", ArrowUp: "↑", ArrowDown: "↓" }[c.key] ?? c.key.toUpperCase();
  return `${c.ctrl ? "⌃" : ""}${c.alt ? "⌥" : ""}${c.shift ? "⇧" : ""}${c.meta ? "⌘" : ""}${key}`;
}

/** Chords the person changed in Settings › Keys, by action id; the rest keep actions.json. */
export type KeyBindings = Record<string, string>;

/** The actions with the person's chords in place of the defaults. */
export function withBindings(actions: readonly ActionDef[], bindings: KeyBindings): ActionDef[] {
  return actions.map((a) => (bindings[a.id] ? { ...a, accel: bindings[a.id]! } : a));
}

/** A chord in one spelling, so `Shift+Cmd+p` and `Cmd+Shift+P` compare equal. */
export function normalAccel(accel: string): string {
  const c = parseAccel(accel);
  return [c.ctrl && "Ctrl", c.alt && "Alt", c.shift && "Shift", c.meta && "Cmd", c.key.toLowerCase()].filter(Boolean).join("+");
}

/** The accelerator a key event spells, as actions.json writes it; none for a lone modifier. */
export function accelOf(e: KeyboardEvent): string | undefined {
  let key: string | undefined;
  if (/^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
  else if (/^Digit[0-9]$/.test(e.code)) key = e.code.slice(5);
  else if (/^F([1-9]|1[0-9])$/.test(e.code)) key = e.code;
  else key = CODE_NAMES[e.code];
  if (!key) return undefined;
  return [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Cmd", key].filter(Boolean).join("+");
}

/** macOS and the Edit menu own these; an action may not take them. */
export const RESERVED: readonly { accel: string; label: string }[] = [
  { accel: "Cmd+Q", label: "Quit" },
  { accel: "Cmd+H", label: "Hide" },
  { accel: "Cmd+Alt+H", label: "Hide Others" },
  { accel: "Cmd+M", label: "Minimize" },
  { accel: "Ctrl+Cmd+F", label: "Full Screen" },
  { accel: "Cmd+Z", label: "Undo" },
  { accel: "Cmd+Shift+Z", label: "Redo" },
  { accel: "Cmd+X", label: "Cut" },
  { accel: "Cmd+C", label: "Copy" },
  { accel: "Cmd+V", label: "Paste" },
  { accel: "Cmd+A", label: "Select All" },
  { accel: "Cmd+Tab", label: "the app switcher" },
  { accel: "Cmd+Space", label: "Spotlight" },
];

/**
 * Why `accel` cannot be `id`'s chord, or undefined when it can (D17): it needs ⌘ so plain keys
 * stay with the pane, and it may not be another action's chord or one macOS keeps.
 */
export function bindingProblem(id: string, accel: string, actions: readonly ActionDef[]): string | undefined {
  const key = parseAccel(accel).key.toLowerCase();
  if (!key || ["cmd", "ctrl", "alt", "shift"].includes(key)) return "A shortcut needs a key besides the modifiers.";
  if (!parseAccel(accel).meta) return "An app shortcut needs ⌘, so plain keys stay with the terminal.";
  const want = normalAccel(accel);
  const reserved = RESERVED.find((r) => normalAccel(r.accel) === want);
  if (reserved) return `${chordLabel(accel)} is ${reserved.label}.`;
  const other = actions.find((a) => a.id !== id && normalAccel(a.accel) === want);
  if (other) return `${chordLabel(accel)} is already ${other.label}.`;
  return undefined;
}

/** Each action whose chord another action also has. */
export function conflicts(actions: readonly ActionDef[]): Set<string> {
  const by = new Map<string, string[]>();
  for (const a of actions) by.set(normalAccel(a.accel), [...(by.get(normalAccel(a.accel)) ?? []), a.id]);
  return new Set([...by.values()].filter((ids) => ids.length > 1).flat());
}

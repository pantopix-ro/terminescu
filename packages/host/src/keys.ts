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

/** The key a chord ends on, from the physical key so ⌥ and ⇧ don't change it. */
function keyOf(e: KeyboardEvent): string {
  if (/^Key[A-Z]$/.test(e.code)) return e.code.slice(3).toLowerCase();
  if (/^Digit[0-9]$/.test(e.code)) return e.code.slice(5);
  if (e.code === "Backslash") return "\\";
  if (e.code === "Comma") return ",";
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

import type { Pin, Variable } from "@terminescu/host";
import { isAskable, resolveName } from "./insert";

/** One flip of the prompt tooltip's placeholders, in ms. */
export const BLINK_MS = 1000;
/** A slot is at least as wide as its placeholder and at most this many characters, unless the placeholder is longer. */
export const SLOT_CAP = 24;
/** Lines of the body the tooltip shows before it clips under a fade. */
export const TIP_LINES = 12;

export type TipPart =
  | { text: string }
  /** `raw` is the `{{ name }}` text as written; `shown` the value; `width` in `ch`. */
  | { raw: string; shown: string; width: number };

/** The body as text runs and slots. A name no value fills stays text and never blinks. */
export function tipSlots(body: string, variables: Variable[], pins: Pin[]): TipPart[] {
  const parts: TipPart[] = [];
  let last = 0;
  const push = (text: string) => {
    if (text) parts.push({ text });
  };
  for (const m of body.matchAll(/\{\{([^{}]*)\}\}/g)) {
    const raw = m[0];
    const name = (m[1] ?? "").trim();
    const asPins = name === "pins" && !variables.some((v) => v.name === name);
    const value = asPins ? `${pins.length} pins` : resolveName(name, variables, pins);
    push(body.slice(last, m.index));
    last = m.index + raw.length;
    if (value === undefined || !(isAskable(name) || asPins)) {
      push(raw);
      continue;
    }
    const flat = value.replace(/\r\n|[\r\n\t]/g, " ");
    const cap = Math.max(raw.length, SLOT_CAP);
    const shown = flat.length > cap ? `${flat.slice(0, cap - 1)}…` : flat;
    parts.push({ raw, shown, width: Math.max(raw.length, Math.min(flat.length, cap)) });
  }
  push(body.slice(last));
  return parts;
}

import type { Pin, Variable } from "@terminescu/host";

const START = "\x1b[200~";
const END = "\x1b[201~";

/**
 * The bytes an insert writes to a terminal (D13). Line breaks become `\n` and the text never
 * carries `\r`, so nothing runs by itself. Text with a line break always goes in as a bracketed
 * paste, even when the program did not ask for one: an unbracketed `\n` would run each line.
 * Paste markers inside the text are dropped so it cannot end the paste early. `send` adds the
 * one `\r` of "insert and send".
 */
export function pasteBytes(text: string, bracketed: boolean, send = false): string {
  const clean = text.replaceAll(START, "").replaceAll(END, "").replace(/\r\n?/g, "\n");
  const wrap = bracketed || clean.includes("\n");
  return (wrap ? `${START}${clean}${END}` : clean) + (send ? "\r" : "");
}

/** What `{{pins}}` becomes: each pin as a Markdown quote. */
export function renderPins(pins: Pin[]): string {
  return pins.map((p) => p.exact.trim().split("\n").map((l) => `> ${l}`).join("\n")).join("\n\n");
}

export interface Rendered {
  text: string;
  /** Names used in the body with no value in the context or `extra`. */
  missing: string[];
}

/**
 * Fills a prompt body from the side tab's context (D13, D21): `{{name}}` takes the variable's
 * value, a secret becomes `$NAME`, `{{pins}}` the pins unless a variable is named `pins`.
 * `extra` holds values typed in the dialog. A missing name stays as `{{name}}`.
 */
export function renderPrompt(body: string, variables: Variable[], pins: Pin[], extra: Record<string, string> = {}): Rendered {
  const missing: string[] = [];
  const text = body.replace(/\{\{([^{}]*)\}\}/g, (whole, raw: string) => {
    const name = raw.trim();
    const v = variables.find((x) => x.name === name);
    if (v) return v.mode === "secret" ? `$${v.name}` : (v.value ?? "");
    if (extra[name]?.length) return extra[name]!;
    if (name === "pins") return renderPins(pins);
    if (/^[A-Za-z_][A-Za-z0-9_]{0,63}$/.test(name) && !missing.includes(name)) missing.push(name);
    return whole;
  });
  return { text, missing };
}

import type { Variable } from "@terminescu/host";

/** A context value found in text that could become `{{name}}` (D14). */
export interface Found {
  name: string;
  /** The text as it appears: the value, or `$NAME` for a secret. */
  value: string;
}

/** Shorter values match too much ordinary text. */
const MIN_VALUE = 3;

/**
 * The context variables whose value appears in `text`, longest value first so a long value
 * wins over one it contains. A secret is found by its `$NAME`, since its value never leaves
 * the daemon.
 */
export function detectVariables(text: string, variables: Variable[]): Found[] {
  const found: Found[] = [];
  for (const v of variables) {
    const value = v.mode === "secret" ? `$${v.name}` : (v.value ?? "").trim();
    if (value.length < MIN_VALUE || !text.includes(value)) continue;
    if (found.some((f) => f.value === value)) continue;
    found.push({ name: v.name, value });
  }
  return found.sort((a, b) => b.value.length - a.value.length);
}

/** Replaces each picked value with `{{name}}`, longest first, never inside a replacement. */
export function templatize(text: string, picks: Found[]): string {
  const order = [...picks].sort((a, b) => b.value.length - a.value.length);
  let parts: (string | { name: string })[] = [text];
  for (const p of order) {
    parts = parts.flatMap((part) => {
      if (typeof part !== "string") return [part];
      const pieces = part.split(p.value);
      return pieces.flatMap((piece, i) => (i === 0 ? [piece] : [{ name: p.name }, piece]));
    });
  }
  return parts.map((p) => (typeof p === "string" ? p : `{{${p.name}}}`)).join("");
}

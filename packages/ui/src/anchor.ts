/** Characters of context kept on each side of a pinned quote (D12). */
export const QUOTE_CONTEXT = 40;

export interface Quote {
  exact: string;
  prefix: string;
  suffix: string;
}

/** The text-quote selector for `[start, end)` of `text`. */
export function quoteOf(text: string, start: number, end: number): Quote {
  return {
    exact: text.slice(start, end),
    prefix: text.slice(Math.max(0, start - QUOTE_CONTEXT), start),
    suffix: text.slice(end, end + QUOTE_CONTEXT),
  };
}

/**
 * Where a pinned quote sits in `text` now: at its offsets when the text there still matches,
 * else the occurrence whose surroundings match the prefix and suffix best (D12).
 */
export function anchor(
  text: string,
  pin: { exact: string; prefix: string | null; suffix: string | null; start: number | null },
): [number, number] | undefined {
  const { exact } = pin;
  if (!exact) return undefined;
  if (pin.start != null && text.slice(pin.start, pin.start + exact.length) === exact) {
    return [pin.start, pin.start + exact.length];
  }
  let best: [number, number] | undefined;
  let bestScore = -1;
  for (let at = text.indexOf(exact); at >= 0; at = text.indexOf(exact, at + 1)) {
    const before = text.slice(0, at);
    const after = text.slice(at + exact.length);
    const score = common(before, pin.prefix ?? "", true) + common(after, pin.suffix ?? "", false);
    if (score > bestScore) {
      bestScore = score;
      best = [at, at + exact.length];
    }
  }
  return best;
}

/** Shared characters at the end (`fromEnd`) or start of two strings. */
function common(a: string, b: string, fromEnd: boolean): number {
  let n = 0;
  while (n < a.length && n < b.length) {
    const x = fromEnd ? a[a.length - 1 - n] : a[n];
    const y = fromEnd ? b[b.length - 1 - n] : b[n];
    if (x !== y) break;
    n++;
  }
  return n;
}

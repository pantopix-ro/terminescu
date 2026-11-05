import { describe, expect, it } from "vitest";
import { anchor, quoteOf } from "../src/anchor";

describe("anchor", () => {
  const text = "Plan: move rows in batches of 5,000 rows on a cursor over id, then verify.";
  const start = text.indexOf("batches");
  const end = start + "batches of 5,000 rows on a cursor over id".length;

  it("finds the quote at its offsets", () => {
    const q = quoteOf(text, start, end);
    expect(anchor(text, { ...q, start })).toEqual([start, end]);
  });
  it("re-anchors by the quote when the offsets drift", () => {
    const q = quoteOf(text, start, end);
    const moved = `Updated. ${text}`;
    expect(anchor(moved, { ...q, start })).toEqual([start + 9, end + 9]);
  });
  it("picks the occurrence whose context matches", () => {
    const twice = "id one: cursor over id. Then: cursor over id, done.";
    const second = twice.lastIndexOf("cursor over id");
    const q = quoteOf(twice, second, second + 14);
    expect(anchor(twice, { ...q, start: null })).toEqual([second, second + 14]);
  });
  it("gives up when the quote is gone", () => {
    expect(anchor("other text", { exact: "missing", prefix: null, suffix: null, start: 0 })).toBeUndefined();
  });
});

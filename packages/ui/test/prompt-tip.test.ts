import type { Pin, Variable } from "@terminescu/host";
import { describe, expect, it } from "vitest";
import { tipSlots } from "../src/prompt-tip";

const v = (name: string, mode: Variable["mode"], value: string | null): Variable => ({
  id: name,
  sideTabId: "st-1",
  name,
  value,
  mode,
  source: "typed",
  originTabId: null,
  expiresAt: null,
});
const pin = (exact: string): Pin => ({ id: exact, exact }) as unknown as Pin;
const slot = (body: string, vars: Variable[], pins: Pin[] = []) => {
  const found = tipSlots(body, vars, pins).find((p) => "raw" in p);
  return found && "raw" in found ? found : undefined;
};

describe("tipSlots", () => {
  it("a short value takes the width of the placeholder", () => {
    expect(slot("{{ticket}}", [v("ticket", "saved", "PROJ-482")])).toEqual({ raw: "{{ticket}}", shown: "PROJ-482", width: 10 });
  });

  it("a value longer than the placeholder grows the slot up to 24, then ends in an ellipsis", () => {
    expect(slot("{{a}}", [v("a", "saved", "x".repeat(20))])?.width).toBe(20);
    const long = slot("{{branch}}", [v("branch", "saved", "f".repeat(40))])!;
    expect(long.width).toBe(24);
    expect(long.shown).toBe(`${"f".repeat(23)}…`);
  });

  it("a placeholder longer than 24 sets the cap", () => {
    const name = "n".repeat(30);
    const s = slot(`{{${name}}}`, [v(name, "saved", "v".repeat(60))])!;
    expect(s.width).toBe(34);
    expect(s.shown.length).toBe(34);
    expect(s.shown.endsWith("…")).toBe(true);
  });

  it("a secret shows $NAME, never its value", () => {
    expect(slot("{{API_TOKEN}}", [v("API_TOKEN", "secret", null)])?.shown).toBe("$API_TOKEN");
  });

  it("pins show a count, unless a variable is named pins", () => {
    expect(slot("{{pins}}", [], [pin("a"), pin("b")])?.shown).toBe("2 pins");
    expect(slot("{{pins}}", [v("pins", "saved", "mine")], [pin("a")])?.shown).toBe("mine");
  });

  it("a missing name stays plain text and never blinks", () => {
    expect(tipSlots("by {{owner}}", [], [])).toEqual([{ text: "by " }, { text: "{{owner}}" }]);
  });

  it("newlines and tabs in a value show as one space", () => {
    expect(slot("{{a}}", [v("a", "saved", "one\r\ntwo\tthree")])?.shown).toBe("one two three");
  });
});

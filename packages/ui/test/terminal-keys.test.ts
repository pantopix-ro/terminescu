import { describe, expect, it } from "vitest";
import { terminalKeyText, type KeyLike } from "../src/terminal-keys";

const key = (k: string, mods: Partial<KeyLike> = {}): KeyLike => ({ key: k, metaKey: false, altKey: false, ctrlKey: false, shiftKey: false, ...mods });

describe("terminalKeyText", () => {
  it("moves a word with ⌥← and ⌥→ as Terminal.app does", () => {
    expect(terminalKeyText(key("ArrowLeft", { altKey: true }))).toBe("\x1bb");
    expect(terminalKeyText(key("ArrowRight", { altKey: true }))).toBe("\x1bf");
  });
  it("sends ^A, ^E and ^U for ⌘←, ⌘→ and ⌘⌫", () => {
    expect(terminalKeyText(key("ArrowLeft", { metaKey: true }))).toBe("\x01");
    expect(terminalKeyText(key("ArrowRight", { metaKey: true }))).toBe("\x05");
    expect(terminalKeyText(key("Backspace", { metaKey: true }))).toBe("\x15");
  });
  it("leaves plain keys, menu chords and selection chords to xterm.js and the menu", () => {
    expect(terminalKeyText(key("ArrowLeft"))).toBeUndefined();
    expect(terminalKeyText(key("ArrowUp", { metaKey: true }))).toBeUndefined();
    expect(terminalKeyText(key("ArrowLeft", { metaKey: true, altKey: true }))).toBeUndefined();
    expect(terminalKeyText(key("ArrowLeft", { altKey: true, shiftKey: true }))).toBeUndefined();
    expect(terminalKeyText(key("Backspace", { altKey: true }))).toBeUndefined();
    expect(terminalKeyText(key("k", { metaKey: true }))).toBeUndefined();
  });
});

import { describe, expect, it } from "vitest";
import { ACTIONS, actionFor, chordLabel } from "../src/keys";

const key = (code: string, mods: Partial<KeyboardEvent> = {}, k = "") =>
  ({ code, key: k, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...mods }) as KeyboardEvent;

describe("chords", () => {
  it("maps ⌘ chords to actions and leaves plain keys alone", () => {
    expect(actionFor(key("KeyT", { metaKey: true }))).toBe("new_tab");
    expect(actionFor(key("KeyD", { metaKey: true }))).toBe("split_right");
    expect(actionFor(key("KeyD", { metaKey: true, shiftKey: true }))).toBe("split_down");
    expect(actionFor(key("Digit3", { metaKey: true }))).toBe("tab_3");
    expect(actionFor(key("Digit3", { metaKey: true, ctrlKey: true }))).toBe("side_3");
    expect(actionFor(key("Backslash", { metaKey: true }, "\\"))).toBe("context_panel");
    expect(actionFor(key("ArrowLeft", { metaKey: true, altKey: true }, "ArrowLeft"))).toBe("focus_left");
    expect(actionFor(key("KeyT"))).toBeUndefined();
    expect(actionFor(key("KeyC", { ctrlKey: true }))).toBeUndefined();
    expect(actionFor(key("KeyC", { metaKey: true }))).toBeUndefined();
  });

  it("has one action per chord", () => {
    const accels = ACTIONS.map((a) => a.accel);
    expect(new Set(accels).size).toBe(accels.length);
  });

  it("labels chords in macOS order", () => {
    expect(chordLabel("Ctrl+Cmd+1")).toBe("⌃⌘1");
    expect(chordLabel("Cmd+Shift+D")).toBe("⇧⌘D");
    expect(chordLabel("Cmd+Alt+Left")).toBe("⌥⌘←");
  });
});

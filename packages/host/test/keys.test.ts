import { describe, expect, it } from "vitest";
import { ACTIONS, accelOf, actionFor, bindingProblem, chordLabel, conflicts, normalAccel, withBindings, type ActionDef } from "../src/keys";

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
    expect(actionFor(key("KeyS", { metaKey: true, shiftKey: true }))).toBe("status_editor");
    expect(actionFor(key("Comma", { metaKey: true }, ","))).toBe("settings");
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
    expect(chordLabel("Cmd+Comma")).toBe("⌘,");
  });
});

// "Open file" arrives with text tabs (7.3); the rebinding logic does not depend on it.
const OPEN_FILE: ActionDef = { id: "open_file", label: "Open File…", menu: "File", accel: "Cmd+O" };
const withOpenFile = [...ACTIONS, OPEN_FILE];

describe("editable shortcuts (task 6.10)", () => {
  it("rebinds Open file to ⌘P: ⌘P opens it and ⌘O does nothing", () => {
    const actions = withBindings(withOpenFile, { open_file: "Cmd+P" });
    expect(bindingProblem("open_file", "Cmd+P", withOpenFile)).toBeUndefined();
    expect(actionFor(key("KeyP", { metaKey: true }), actions)).toBe("open_file");
    expect(actionFor(key("KeyO", { metaKey: true }), actions)).toBeUndefined();
    expect(actionFor(key("KeyO", { metaKey: true }), withOpenFile)).toBe("open_file");
  });

  it("flags ⌘T as a clash, and a chord without ⌘ or one macOS keeps", () => {
    expect(bindingProblem("open_file", "Cmd+T", withOpenFile)).toBe("⌘T is already New Tab.");
    expect(bindingProblem("open_file", "Shift+Cmd+t", withOpenFile)).toBeUndefined();
    expect(bindingProblem("open_file", "Ctrl+P", withOpenFile)).toMatch(/needs ⌘/);
    expect(bindingProblem("open_file", "Cmd+C", withOpenFile)).toBe("⌘C is Copy.");
    expect(bindingProblem("open_file", "Cmd++", withOpenFile)).toMatch(/needs a key/);
    expect(bindingProblem("open_file", "Cmd+Shift", withOpenFile)).toMatch(/needs a key/);
    // Its own chord is no clash.
    expect(bindingProblem("new_tab", "Cmd+T", withOpenFile)).toBeUndefined();
    expect([...conflicts(withBindings(withOpenFile, { open_file: "Cmd+T" }))].sort()).toEqual(["new_tab", "open_file"]);
    expect(conflicts(withOpenFile).size).toBe(0);
  });

  it("⌘D in a text pane still splits: the chord wins over the editor", () => {
    const editor = { ...key("KeyD", { metaKey: true }), target: { closest: () => ({ className: "cm-editor" }) } } as unknown as KeyboardEvent;
    expect(actionFor(editor)).toBe("split_right");
  });

  it("spells a key event as an accelerator and compares chords in any order", () => {
    expect(accelOf(key("KeyP", { metaKey: true, shiftKey: true }, "P"))).toBe("Shift+Cmd+P");
    expect(accelOf(key("Period", { metaKey: true }, "."))).toBe("Cmd+Period");
    expect(accelOf(key("ArrowUp", { metaKey: true, altKey: true }))).toBe("Alt+Cmd+Up");
    expect(accelOf(key("MetaLeft", { metaKey: true }, "Meta"))).toBeUndefined();
    expect(normalAccel("Shift+Cmd+P")).toBe(normalAccel("Cmd+Shift+p"));
    // A recorded chord matches its own key event.
    const e = key("Period", { metaKey: true }, ".");
    expect(actionFor(e, withBindings(ACTIONS, { palette: accelOf(e)! }))).toBe("palette");
  });
});

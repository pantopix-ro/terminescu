// The Rust half's recorded answers (polish-claude-insights D8) read through the UI's types: a
// field the UI expects and Rust no longer sends fails here before the e2e fakes drift.

import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import type { ClaudeChanges, ClaudeSettings, TodosAnswer } from "../src";
import { message } from "../src/SettingsPage";

const contract = <T>(name: string): T =>
  JSON.parse(readFileSync(new URL(`../contract/${name}.json`, import.meta.url), "utf8")) as T;

const STATES = ["pending", "inProgress", "completed"];

describe("the contract fixtures", () => {
  it("settings carry every field the page reads", () => {
    const { view, errors } = contract<{ view: ClaudeSettings; errors: Record<string, string> }>("settings");
    for (const k of ["meterThreshold", "baselineCapMb", "baselineDays", "baselineFileMb"] as const) {
      expect(typeof view[k]).toBe("number");
      expect(typeof view.defaults[k]).toBe("number");
    }
    expect(typeof view.defaults.window).toBe("number");
    expect(view.contextWindows).toEqual({});
    expect(message(new Error(errors.meterThreshold!))).toBe("the threshold is a whole number from 1 to 100");
  });

  it("todos name a session and states the section knows", () => {
    const { none, some } = contract<{ none: TodosAnswer; some: TodosAnswer }>("todos");
    expect(none).toEqual({ session: null, todos: null });
    expect(typeof some.session).toBe("string");
    for (const t of some.todos ?? []) {
      expect(STATES).toContain(t.state);
      expect(typeof t.content).toBe("string");
    }
  });

  it("changes and base texts have the shapes the section and the diff read", () => {
    const { changes, baseText } = contract<{ changes: ClaudeChanges; baseText: Record<string, Record<string, unknown>> }>("changes");
    expect(typeof changes.session).toBe("string");
    expect(typeof changes.tree).toBe("boolean");
    expect(changes.files.every((f) => f.path.startsWith(`${changes.base}/`))).toBe(true);
    for (const f of changes.files) {
      expect(typeof f.created).toBe("boolean");
      expect(typeof f.deleted).toBe("boolean");
    }
    for (const b of Object.values(baseText)) {
      for (const k of ["exists", "binary", "large"]) expect(typeof b[k]).toBe("boolean");
    }
    expect(baseText.created?.text).toBe("");
  });
});

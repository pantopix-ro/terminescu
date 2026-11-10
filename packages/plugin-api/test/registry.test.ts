import { describe, expect, it } from "vitest";
import type { ActionDef, Decoration } from "@terminescu/host";
import { compose, definePlugin, toItems, type Decorator, type RowView } from "../src";

const deco = (plugin: string, kind: Decoration["target"]["kind"], id: string, key: string, text: string): Decoration => ({
  plugin,
  target: { kind, id },
  slot: "chip",
  key,
  text,
  tone: "neutral",
});

const app: ActionDef[] = [
  { id: "new_tab", label: "New Tab", menu: "Shell", accel: "Cmd+T" },
  { id: "palette", label: "Command Palette", menu: "View", accel: "Cmd+Shift+P" },
];

const noop = () => {};
const View = () => null;

describe("compose", () => {
  // A folder row shows its shown tab's chips: git's row decorator reads its tab decorations.
  const shownTabChips: Decorator<RowView> = (items, row, mine) => [
    ...items,
    ...toItems(mine.filter((d) => d.target.kind === "tab" && d.target.id === row.shownTabId)),
  ];
  const git = definePlugin({
    id: "git",
    name: "Git",
    rowDecorators: [shownTabChips],
    actions: [
      { id: "worktree", label: "New Worktree Row", accel: "Cmd+Alt+W", run: noop },
      { id: "steal", label: "Steal New Tab", accel: "Cmd+T", run: noop },
    ],
    paneViews: [{ kind: "diff", render: View }],
  });
  const claude = definePlugin({
    id: "claude",
    name: "Claude",
    // Tries to drop everyone's items; only its own output counts.
    rowDecorators: [(_items, row, mine) => toItems(mine.filter((d) => d.target.id === row.id))],
    actions: [
      { id: "focus", label: "Focus Waiting", accel: "alt+Cmd+w", run: noop },
      { id: "plain", label: "Plain Key", accel: "Alt+W", run: noop },
    ],
    paneViews: [{ kind: "transcript", render: View }, { kind: "diff", render: View }],
    panelSections: [{ id: "usage", title: "Usage", render: View }],
  });
  const decorations = [
    deco("git", "tab", "t1", "branch", "main"),
    deco("git", "sideTab", "s1", "ignored", "row chip git does not show"),
    deco("claude", "sideTab", "s1", "meter", "40%"),
    deco("claude", "tab", "t1", "tab-only", "x"),
    deco("off", "sideTab", "s1", "x", "from a disabled plugin"),
  ];
  const all = compose({ plugins: [git, claude], enabled: new Set(["git", "claude"]), decorations, appActions: app });
  const row = { id: "s1", tabIds: ["t1"], shownTabId: "t1" };

  it("composes decorators in plugin order and keeps other plugins' items", () => {
    expect(all.rowItems(row).map((i) => `${i.plugin}:${i.text}`)).toEqual(["git:main", "claude:40%"]);
  });

  it("shows a plugin's own tab decorations when it has no tab decorator", () => {
    expect(all.tabItems({ id: "t1", sideTabId: "s1", paneIds: [] }).map((i) => i.text)).toEqual(["main", "x"]);
  });

  it("gives the app and earlier plugins the chord", () => {
    const by = new Map(all.actions.map((a) => [a.id, a]));
    expect(by.get("git.worktree")?.accel).toBe("Cmd+Alt+W");
    expect(by.get("git.steal")).toMatchObject({ accel: "", lostAccel: "Cmd+T" });
    expect(by.get("claude.focus")).toMatchObject({ accel: "", lostAccel: "alt+Cmd+w" });
    expect(by.get("claude.plain")).toMatchObject({ accel: "", lostAccel: "Alt+W" });
    expect(by.get("git.worktree")?.menu).toBe("Plugins");
  });

  it("leaves a disabled plugin out and names the views it would draw", () => {
    const some = compose({ plugins: [git, claude], enabled: new Set(["git"]), decorations, appActions: app });
    expect(some.rowItems(row).map((i) => i.plugin)).toEqual(["git"]);
    expect(some.actions.map((a) => a.plugin)).toEqual(["git", "git"]);
    expect(some.panelSections).toEqual([]);
    expect(some.paneView("transcript")).toBeUndefined();
    expect([...some.offKinds]).toEqual(["transcript"]);
    expect(some.paneView("diff")?.plugin).toBe("git");
    expect(all.paneView("diff")?.plugin).toBe("git");
    expect(all.offKinds.size).toBe(0);
  });

  it("shows a single row's tab decorations on the row, not a folder's", () => {
    const plain = definePlugin({ id: "git", name: "Git" });
    const some = compose({ plugins: [plain], enabled: new Set(["git"]), decorations, appActions: app });
    expect(some.rowItems(row).map((i) => i.text)).toEqual(["main", "row chip git does not show"]);
    expect(some.rowItems({ id: "s1", tabIds: ["t1", "t2"], shownTabId: "t1" }).map((i) => i.text)).toEqual(["row chip git does not show"]);
  });

  it("lists panel sections with their plugin", () => {
    expect(all.panelSections.map((s) => `${s.plugin}.${s.id}`)).toEqual(["claude.usage"]);
  });
});

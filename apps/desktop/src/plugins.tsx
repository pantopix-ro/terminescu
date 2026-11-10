// The plugins' UI halves and their composition (add-plugin-architecture D7, D8). The app reads
// only the composed lists; which plugins exist is decided here and in the Rust build.

import { useEffect, useMemo, useState } from "react";
import type { ActionDef, Decoration, Host, PluginInfo } from "@terminescu/host";
import type { FakePlugin } from "@terminescu/host/mock";
import { compose, definePlugin, type Composed, type PluginBridge, type UiPlugin } from "@terminescu/plugin-api";
import { claude } from "@terminescu/plugin-claude";
import { git } from "@terminescu/plugin-git";

/** `VITE_PLUGINS=none` builds the bare terminal; the bundler drops the unused halves. */
export const NO_PLUGINS = import.meta.env.VITE_PLUGINS === "none";

/** The UI halves built into the app, in plugin order. */
const BUILT_IN: readonly UiPlugin[] = NO_PLUGINS ? [] : [claude, git];

/** A bridge to one plugin's Rust half. */
export function bridgeOf(host: Host, plugin: string): PluginBridge {
  return {
    call: (method, args) => host.pluginCall(plugin, method, args),
    on(event, listener) {
      let off: (() => void) | undefined;
      let gone = false;
      void host
        .onPluginEvent(plugin, (e, payload) => e === event && listener(payload))
        .then((o) => (gone ? o() : (off = o)));
      return () => {
        gone = true;
        off?.();
      };
    },
  };
}

/** The plugins in plugin order: a Rust half without a UI half still shows its decorations. */
function uiPlugins(infos: readonly PluginInfo[], halves: readonly UiPlugin[]): UiPlugin[] {
  return infos.map((i) => halves.find((p) => p.id === i.id) ?? definePlugin({ id: i.id, name: i.name }));
}

export interface Plugins {
  infos: PluginInfo[];
  composed: Composed;
  /** The name of the plugin that draws `kind`, when it is installed but off. */
  offOwner(kind: string): string | undefined;
  bridge(plugin: string): PluginBridge;
}

/** Follows the plugin list and the decorations and composes the enabled plugins (D7). */
export function usePlugins(host: Host, appActions: readonly ActionDef[], halves: readonly UiPlugin[] = HALVES): Plugins {
  const [infos, setInfos] = useState<PluginInfo[]>([]);
  const [decorations, setDecorations] = useState<Decoration[]>([]);
  useEffect(() => {
    let gone = false;
    const offs: (() => void)[] = [];
    const keep = (p: Promise<() => void>) => void p.then((o) => (gone ? o() : offs.push(o)), () => undefined);
    keep(host.onPlugins((p) => !gone && setInfos(p)));
    keep(host.onDecorations((d) => !gone && setDecorations(d)));
    void host.plugins().then((p) => !gone && setInfos(p), () => undefined);
    void host.decorations().then((d) => !gone && setDecorations(d), () => undefined);
    return () => {
      gone = true;
      offs.forEach((o) => o());
    };
  }, [host]);
  const plugins = useMemo(() => uiPlugins(infos, halves), [infos, halves]);
  const enabled = useMemo(() => new Set(infos.filter((i) => i.enabled && !i.paused).map((i) => i.id)), [infos]);
  const bridges = useMemo(() => new Map<string, PluginBridge>(), [host]);
  const bridge = (plugin: string) => {
    let b = bridges.get(plugin);
    if (!b) bridges.set(plugin, (b = bridgeOf(host, plugin)));
    return b;
  };
  // Each enabled plugin's raised flags, from its own follower; a plugin turned off drops them.
  const [flags, setFlags] = useState<ReadonlyMap<string, ReadonlySet<string>>>(new Map());
  const following = plugins
    .filter((p) => p.flags && enabled.has(p.id))
    .map((p) => p.id)
    .join(",");
  useEffect(() => {
    const ids = new Set(following.split(",").filter(Boolean));
    // Flags of a plugin still followed stay up while its follower starts again, so its gated
    // items do not blink; a plugin turned off drops them.
    setFlags((m) => ([...m.keys()].every((id) => ids.has(id)) ? m : new Map([...m].filter(([id]) => ids.has(id)))));
    const stops = plugins
      .filter((p) => p.flags && ids.has(p.id))
      .map((p) => {
        try {
          return p.flags!(bridge(p.id), (raised) =>
            setFlags((m) => {
              const next = new Set(raised);
              const was = m.get(p.id);
              if (was && was.size === next.size && [...next].every((f) => was.has(f))) return m;
              return new Map(m).set(p.id, next);
            }),
          );
        } catch (e) {
          // A plugin whose follower throws raises no flags; the app goes on.
          console.error(`plugin ${p.id}: flags`, e);
          return () => undefined;
        }
      });
    return () => stops.forEach((s) => s());
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [host, following]);
  const composed = useMemo(() => compose({ plugins, enabled, decorations, appActions, flags }), [plugins, enabled, decorations, appActions, flags]);
  // The native menu (and the mock's chords) follow the composed actions.
  const menuKey = composed.actions.map((a) => `${a.id}|${a.label}|${a.menu}|${a.accel}`).join("\n");
  useEffect(() => {
    void host.setPluginActions(composed.actions.map(({ id, label, menu, accel }) => ({ id, label, menu, accel }))).catch(() => undefined);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [host, menuKey]);
  return {
    infos,
    composed,
    offOwner: (kind) => (composed.offKinds.has(kind) ? plugins.find((p) => p.paneViews?.some((v) => v.kind === kind))?.name : undefined),
    bridge,
  };
}

// ---- Fake Rust halves: the browser build only. `?plugins=fake` plays every slot for
// e2e/plugins.spec.ts; `?plugins=git` plays the git plugin's Rust half for e2e/git.spec.ts;
// `?plugins=claude` the Claude half's insight calls for e2e/claude.spec.ts.

const asked = (name: string) => typeof location !== "undefined" && (new URLSearchParams(location.search).get("plugins") ?? "").split(",").includes(name);
const fakeOn = () => asked("fake");

/**
 * What the fake git half answers, as a test sets it on `window.__fakeGit`: by pane for
 * `changes` and the branches `openWorktrees` finds without a row, by working tree root for
 * `baseText` (a path missing there is a new file), by row for a worktree's `changed` files.
 */
interface FakeGit {
  changes?: { root: string };
  /** The branch's open pull request, for `facts`. */
  pr?: unknown;
  base?: Record<string, string>;
  worktrees?: string[];
  changed?: number;
  /** The forced remove fails too. */
  stuck?: boolean;
}

/**
 * GitHub as `gh` would answer it, on `window.__fakeGitHub`: absent plays no `gh`. `prs` are
 * the repository's open pull requests.
 */
interface FakeGitHub {
  prs: { number: number; title: string; url: string; branch: string; checks: string }[];
}

/** What the fake did with worktrees, for the tests to read on `window.__fakeGitDone`. */
interface FakeGitDone {
  added: { branch: string; base: string | null }[];
  checkedOut: number[];
  removed: { row: string; force: boolean }[];
  /** The marked rows: a worktree row's id and its folder. */
  marks: Record<string, string>;
}

const fakeGit = (): FakePlugin => ({
  id: "git",
  name: "Git",
  description: "Plays the git plugin's Rust half, for the tests.",
  async call(method, args, host) {
    const a = (args ?? {}) as { pane?: string; root?: string; path?: string; row?: string; branch?: string; base?: string | null; force?: boolean; number?: number };
    const github = (globalThis as { __fakeGitHub?: FakeGitHub }).__fakeGitHub;
    const all = (globalThis as { __fakeGit?: Record<string, FakeGit> }).__fakeGit ?? {};
    const answers = all[a.pane ?? a.root ?? a.row ?? ""];
    const done = ((globalThis as { __fakeGitDone?: FakeGitDone }).__fakeGitDone ??= { added: [], checkedOut: [], removed: [], marks: {} });
    // A row below the pane's, as the core's `new_row` makes it, marked as a worktree row.
    const newRow = async (title: string, folder: string) => {
      const ws = host.workspace();
      const win = ws.windows.find((w) => w.sideTabs.some((sd) => sd.tabs.some((t) => JSON.stringify(t.layout).includes(`"${a.pane}"`))));
      if (!win) throw new Error(`no pane ${a.pane}`);
      const at = win.sideTabs.findIndex((sd) => sd.tabs.some((t) => JSON.stringify(t.layout).includes(`"${a.pane}"`)));
      const before = new Set(win.sideTabs.map((sd) => sd.id));
      await host.send({ type: "CreateSideTab", windowId: win.id, index: at + 1 });
      const row = host.workspace().windows.find((w) => w.id === win.id)!.sideTabs.find((sd) => !before.has(sd.id))!;
      await host.send({ type: "RenameSideTab", sideTabId: row.id, title });
      done.marks[row.id] = folder;
      return row.id;
    };
    const repo = () => {
      if (!answers?.changes) throw new Error("the pane is not in a git repository");
      return answers.changes.root;
    };
    if (method === "changes") {
      if (!answers?.changes) throw new Error("the pane is not in a git repository");
      return answers.changes;
    }
    if (method === "settings" || method === "setSetting") {
      const KEY = "terminescu.fakeGit.settings";
      const kept = JSON.parse(localStorage.getItem(KEY) ?? "{}") as { worktreeFolder?: string | null; github?: boolean };
      if (method === "setSetting") {
        const { key, value } = args as { key: string; value: unknown };
        if (key === "worktreeFolder") {
          const v = typeof value === "string" ? value.trim() : "";
          if (v && !(v === "~" || v.startsWith("~/") || v.startsWith("/"))) throw new Error("the folder is a full path, or starts with ~/");
          kept.worktreeFolder = v || null;
        } else if (key === "github") kept.github = value === true;
        else throw new Error(`no setting ${key}`);
        localStorage.setItem(KEY, JSON.stringify(kept));
        if (key === "github") host.pluginEmit("git", "github", { on: !!github && kept.github !== false });
      }
      const on = !!github && kept.github !== false;
      return {
        worktreeFolder: kept.worktreeFolder ?? null,
        github: kept.github !== false,
        githubOn: on,
        githubOff: on ? null : kept.github === false ? "GitHub is off in Settings" : "gh is not installed",
        version: "2.47.0",
      };
    }
    if (method === "status") return { version: "2.47.0", stopped: null, github: !!github, githubOff: github ? null : "gh is not installed" };
    if (method === "facts") {
      repo();
      return { facts: {}, pr: answers?.pr ?? null };
    }
    if (method === "pullRequests") {
      if (!github) throw new Error("gh is not installed");
      repo();
      return github.prs;
    }
    if (method === "prCheckout") {
      if (!github) throw new Error("gh is not installed");
      const root = repo();
      done.checkedOut.push(a.number ?? 0);
      const path = `${root}-${a.branch}`;
      return { row: await newRow(`#${a.number} ${a.branch}`, path), path };
    }
    if (method === "worktreeAdd") {
      const path = `${repo()}-${a.branch}`;
      done.added.push({ branch: a.branch ?? "", base: a.base ?? null });
      return { row: await newRow(a.branch ?? "", path), path, branch: a.branch };
    }
    if (method === "openWorktrees") {
      const root = repo();
      const opened = [];
      for (const b of answers?.worktrees ?? []) opened.push({ row: await newRow(b, `${root}-${b}`), title: b });
      return { opened };
    }
    if (method === "rowWorktree") {
      const worktree = done.marks[a.row ?? ""];
      return worktree ? { worktree, changed: answers?.changed ?? 0 } : null;
    }
    if (method === "worktreeRemove") {
      if (!done.marks[a.row ?? ""]) throw new Error("the row is not a worktree row");
      const changed = answers?.changed ?? 0;
      if (changed > 0 && !a.force) return { removed: false, changed };
      // `stuck` plays a worktree git cannot remove even with --force (a locked one).
      if (answers?.stuck) return { removed: false, changed };
      done.removed.push({ row: a.row ?? "", force: a.force === true });
      delete done.marks[a.row ?? ""];
      return { removed: true };
    }
    if (method === "baseText") {
      if (!answers?.base) throw new Error("the folder is not in a git repository");
      const text = answers.base[a.path ?? ""];
      return text === undefined ? { root: a.root, exists: false } : { root: a.root, exists: true, binary: false, size: text.length, text };
    }
    throw new Error(`${method} is not a method of this plugin`);
  },
});

/**
 * The Claude half's insight calls, as a test sets them on `window.__fakeClaude`: by pane what
 * `todos` answers, by row the `history` that `resume` reads (add-claude-insights groups 2 and
 * 3), and by row its folder `root` and newest `transcript`; by pane its `changes`; and on
 * `window.__fakeClaudeBases` each path's text before the session. The mock's own Claude half
 * keeps `read`.
 */
const fakeClaude = (): FakePlugin => ({
  id: "claude",
  name: "Claude",
  async call(method, args, host) {
    const a = (args ?? {}) as { pane?: string; row?: string; session?: string };
    const all = (globalThis as { __fakeClaude?: Record<string, { todos?: unknown; changes?: unknown; history?: { id: string; cwd: string }[]; root?: string; transcript?: string }> }).__fakeClaude ?? {};
    if (method === "todos") return all[a.pane ?? ""]?.todos ?? { session: null, todos: null };
    if (method === "changes") {
      const c = all[a.pane ?? ""]?.changes;
      if (!c) throw new Error("no Claude session");
      return c;
    }
    if (method === "baseText") {
      const path = (args as { path?: string }).path ?? "";
      const base = (globalThis as { __fakeClaudeBases?: Record<string, unknown> }).__fakeClaudeBases?.[path];
      if (!base) throw new Error("no copy of the file");
      return base;
    }
    if (method === "history") return all[a.row ?? ""]?.history ?? [];
    if (method === "resume") {
      const s = all[a.row ?? ""]?.history?.find((h) => h.id === a.session);
      if (!s) throw new Error(`${a.session} is not a session of this row`);
      await host.typeInNewTab(a.row!, s.cwd, `claude --resume ${s.id}`);
      return null;
    }
    // As crates/plugin-claude/src/settings.rs, kept in localStorage across a reload.
    if (method === "settings" || method === "setSetting") {
      const KEY = "terminescu.fakeClaude.settings";
      const kept = JSON.parse(localStorage.getItem(KEY) ?? "{}") as Record<string, unknown>;
      const ranges: Record<string, [number, number]> = { meterThreshold: [1, 100], baselineCapMb: [10, 100000], baselineDays: [1, 365] };
      if (method === "setSetting") {
        const { key, value } = args as { key: string; value: unknown };
        if (key === "contextWindows") {
          const m = (value ?? {}) as Record<string, number>;
          for (const n of Object.values(m)) if (!Number.isInteger(n) || n < 1000 || n > 10000000) throw new Error("a window is a whole number from 1000 to 10000000");
          kept[key] = Object.keys(m).length ? m : undefined;
        } else if (ranges[key]) {
          const [lo, hi] = ranges[key];
          if (value !== null && !(Number.isInteger(value) && (value as number) >= lo && (value as number) <= hi))
            throw new Error(`${key === "meterThreshold" ? "the threshold" : key === "baselineCapMb" ? "the cap" : "the days"} is a whole number from ${lo} to ${hi}`);
          kept[key] = value ?? undefined;
        } else throw new Error(`no setting ${key}`);
        localStorage.setItem(KEY, JSON.stringify(kept));
      }
      return {
        meterThreshold: kept.meterThreshold ?? 80,
        contextWindows: kept.contextWindows ?? {},
        baselineCapMb: kept.baselineCapMb ?? 500,
        baselineDays: kept.baselineDays ?? 7,
        defaults: { meterThreshold: 80, window: 200000, baselineCapMb: 500, baselineDays: 7 },
      };
    }
    // The row's folder is its `root`; the Rust test covers finding the repository root.
    if (method === "memory") {
      const which = (args as { which?: string }).which;
      const path = which === "user" ? "/Users/owner/.claude/CLAUDE.md" : `${all[a.row ?? ""]?.root ?? "/Users/owner"}/CLAUDE.md`;
      await host.openTextFor(a.row!, path);
      return { path };
    }
    if (method === "revealTranscripts") {
      const path = all[a.row ?? ""]?.transcript;
      if (!path) throw new Error("Claude has written no transcript yet");
      ((globalThis as { __fakeClaudeRevealed?: string[] }).__fakeClaudeRevealed ??= []).push(path);
      return { path };
    }
    throw new Error(`${method} is not a method of this plugin`);
  },
});

/** The fake plugins' Rust halves, played by the mock host. */
export function fakeRustHalves(): FakePlugin[] {
  const halves = [...(asked("git") ? [fakeGit()] : []), ...(asked("claude") ? [fakeClaude()] : [])];
  if (!fakeOn()) return halves;
  return [
    ...halves,
    {
      id: "fake",
      name: "Fake",
      description: "Plays every slot, for the tests.",
      call(method, args, host) {
        if (method === "ping") return "pong";
        if (method === "hello") {
          const row = (args as { sideTabId?: string } | null)?.sideTabId;
          if (row) host.decorate({ plugin: "fake", target: { kind: "sideTab", id: row }, slot: "chip", key: "hello", text: "hi", tone: "good" });
          return null;
        }
        throw new Error(`${method} is not a method of this plugin`);
      },
    },
    { id: "viewer", name: "Viewer", description: "Draws transcript panes, for the tests." },
  ];
}

function FakeSection({ bridge }: { sideTabId: string; bridge: PluginBridge }) {
  const [answer, setAnswer] = useState<string>();
  return (
    <div className="fake-section small">
      <button type="button" className="ao-btn ghost" onClick={() => void bridge.call<string>("ping").then(setAnswer, (e: unknown) => setAnswer(String(e)))}>
        Ping
      </button>
      {answer && <span className="fake-answer">{answer}</span>}
    </div>
  );
}

function FakeView({ paneId }: { paneId: string }) {
  return <div className="fake-view">Fake view of {paneId}</div>;
}

const FAKE_HALVES: readonly UiPlugin[] = [
  definePlugin({
    id: "fake",
    name: "Fake",
    panelSections: [{ id: "notes", title: "Fake Notes", render: FakeSection }],
    settingsPage: () => <p className="fake-settings small">Fake settings</p>,
    actions: [
      { id: "hello", label: "Fake Hello", accel: "Cmd+Alt+J", run: (cx) => void cx.bridge.call("hello", { sideTabId: cx.sideTabId ?? null }) },
      { id: "steal", label: "Fake New Tab", accel: "Cmd+T", run: () => undefined },
    ],
  }),
  definePlugin({ id: "viewer", name: "Viewer", paneViews: [{ kind: "transcript", render: FakeView }] }),
];

/** The UI halves of this build. */
const HALVES: readonly UiPlugin[] = fakeOn() ? [...BUILT_IN, ...FAKE_HALVES] : BUILT_IN;

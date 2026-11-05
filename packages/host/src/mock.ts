// A Host without Tauri, for unit tests and Playwright runs of the Vite build (task 1.3). It
// plans commands like crates/core/src/state.rs, with readable ids (m1, m2, ...).

import type { Host, PaneView } from "./host";
import { actionFor } from "./keys";
import { hasSplit, isSessionId, leaf, panesOf, removePane, resizeSplit, splitPane, terminalsOf } from "./layout";
import {
  ACCENTS,
  MAX_LABEL,
  TINTS,
  type AgentUpdate,
  type AgentView,
  type Command,
  type Entry,
  type Id,
  type Look,
  type ManualStatus,
  type Pin,
  type Variable,
  type CommandError,
  type Patch,
  type PatchBatch,
  type Rule,
  type SavedStatus,
  type Shown,
  type SideTab,
  type Snapshot,
  type StatusConfig,
  type Tab,
  type Workspace,
} from "./protocol";
import { applyBatch } from "./reducer";
import { computeStatuses, type PaneDynamic, type PaneFacts } from "./status";

export class MockCommandError extends Error {
  constructor(readonly error: CommandError) {
    super(`${error.code} ${error.id}`);
  }
}

export interface MockHost extends Host {
  /** Every batch the mock produced, in order. */
  readonly batches: PatchBatch[];
  workspace(): Workspace;
  /** Links the UI asked to open. */
  readonly openedUrls: string[];
  /** Every size a pane view reported, in order, as `pane cols x rows`. */
  readonly resizes: string[];
  /** Text a ResumePane typed, per pane. */
  readonly typed: Map<string, string>;
  /** Plays the core's part for hook events: sets or clears a pane's agent. */
  setAgent(pane: string, agent: AgentView | null): void;
  /** Appends entries to a session's fake transcript. */
  appendTranscript(session: string, entries: Entry[]): void;
  /**
   * Plays the shell integration: `command` starts in the pane and, unless `exit` is null, ends
   * with that code. Exit-code rules fire as in the core.
   */
  runCommand(pane: string, command: string, exit: number | null): void;
  /**
   * Plays `terminescu status` in a pane; `null` is `--clear`. The mock has no `--for` or
   * `--until-exit` and no Claude idle rules; `crates/core/tests/rules.rs` covers those.
   */
  setCliStatus(pane: string, look: Look | null): void;
}

/** `*` matches any run of characters, `?` one; the whole text must match (as `glob` in the core). */
export function glob(pattern: string, text: string): boolean {
  const re = pattern.replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*").replace(/\?/g, ".");
  return new RegExp(`^${re}$`, "s").test(text);
}

/** The core's `Look::check`, with the same messages. */
export function lookError(look: Look): string | undefined {
  const label = look.label.trim();
  if (!label) return "a status needs a label";
  if ([...label].length > MAX_LABEL) return `a status label has at most ${MAX_LABEL} characters`;
  if (look.accent !== null && !(ACCENTS as readonly string[]).includes(look.accent)) return `${look.accent} is not a status accent`;
  if (look.background !== null && !(TINTS as readonly string[]).includes(look.background)) {
    return `${look.background} is not a status background`;
  }
  return undefined;
}

const encoder = new TextEncoder();

/**
 * A pretend shell per tab: a prompt, local echo and one canned answer per line. Enough for the
 * UI and Playwright to see a pane render, type and resize without a daemon.
 */
class MockShell {
  private line = "";
  private size = "80x24";
  constructor(private readonly paneId: string) {}
  prompt = () => `mock ${this.paneId} $ `;
  redraw = () => `\x1bc${this.prompt()}${this.line}`;
  resize(cols: number, rows: number) {
    this.size = `${cols}x${rows}`;
  }
  input(data: string): string {
    let out = "";
    for (const ch of data) {
      if (ch === "\r") {
        const said = this.line.trim();
        this.line = "";
        const answer = said === "size" ? this.size : said ? `you typed: ${said}` : "";
        out += `\r\n${answer ? `${answer}\r\n` : ""}${this.prompt()}`;
      } else if (ch === "\x7f") {
        if (this.line) {
          this.line = this.line.slice(0, -1);
          out += "\b \b";
        }
      } else if (ch >= " ") {
        this.line += ch;
        out += ch;
      }
    }
    return out;
  }
}

export function createMockHost(initial?: Workspace): MockHost {
  let n = 0;
  const id = () => `m${++n}`;
  let rev = 1;
  let ws: Workspace = initial ? structuredClone(initial) : firstStart();
  const listeners = new Set<(b: PatchBatch) => void>();
  const batches: PatchBatch[] = [];
  const shells = new Map<string, MockShell>();
  const openedUrls: string[] = [];
  const resizes: string[] = [];
  const typed = new Map<string, string>();
  const agents = new Map<string, AgentView>();
  const agentListeners = new Set<(u: AgentUpdate) => void>();
  const transcripts = new Map<string, Entry[]>();
  const pinList: Pin[] = [];
  const vars: Variable[] = [];
  const contextListeners = new Set<(sideTabId: string) => void>();
  const contextChanged = (side: string) => {
    for (const l of contextListeners) l(side);
  };
  // Statuses (D15): shell facts and dynamic statuses per pane, the saved config, and the last
  // map sent, so listeners hear only changes.
  const shellFacts = new Map<string, NonNullable<PaneFacts["shell"]> & { command: string }>();
  // One dynamic status per pane and origin (a rule id, or "cli"), as the core keeps them.
  type Dyn = PaneDynamic & { pane: string; ruleId?: Id };
  const dynamic = new Map<string, Dyn>();
  const dynKey = (pane: string, origin: string) => `${pane} ${origin}`;
  const newestByPane = () => {
    const out = new Map<string, PaneDynamic>();
    for (const d of dynamic.values()) if ((out.get(d.pane)?.order ?? -1) < d.order) out.set(d.pane, d);
    return out;
  };
  let order = 0;
  let config: StatusConfig = { statuses: [], rules: [], builtins: {} };
  let shown: Record<Id, Shown> = {};
  const statusListeners = new Set<(s: Record<Id, Shown>) => void>();
  const configListeners = new Set<() => void>();
  const recompute = () => {
    const facts = new Map<string, PaneFacts>();
    for (const [p, a] of agents) facts.set(p, { agent: a.state });
    for (const [p, sh] of shellFacts) facts.set(p, { ...facts.get(p), shell: sh });
    const next = computeStatuses(ws, facts, newestByPane());
    if (JSON.stringify(next) === JSON.stringify(shown)) return;
    shown = next;
    for (const l of statusListeners) l(structuredClone(next));
  };
  const configChanged = () => {
    // A rule carries its status's look, as `rules()` in the store joins it.
    config.rules = config.rules.flatMap((r) => {
      const st = config.statuses.find((x) => x.id === r.statusId);
      return st ? [{ ...r, look: { label: st.label, accent: st.accent, background: st.background } }] : [];
    });
    for (const [k, d] of dynamic) {
      const rule = d.ruleId && config.rules.find((r) => r.id === d.ruleId && r.enabled);
      if (d.ruleId && !rule) dynamic.delete(k);
      else if (rule) d.look = rule.look;
    }
    recompute();
    for (const l of configListeners) l();
  };
  const checked = (look: Look): Look => {
    const err = lookError(look);
    if (err) throw new Error(err);
    return { label: look.label.trim(), accent: look.accent, background: look.background };
  };
  const savedStatus = (sid: Id) => config.statuses.find((x) => x.id === sid) ?? (() => {
    throw new Error(`no status ${sid}`);
  })();

  function firstStart(): Workspace {
    const windowId = id();
    return { windows: [{ id: windowId, sideTabs: [newSideTab()] }] };
  }
  function newTab(): Tab {
    const tabId = id();
    return { id: tabId, title: null, layout: { type: "pane", id: id() } };
  }
  function newSideTab(): SideTab {
    const tab = newTab();
    return { id: id(), title: null, tabs: [tab] };
  }

  const fail = (code: CommandError["code"], ref: string | number): never => {
    throw new MockCommandError({ code, id: ref });
  };
  const allSideTabs = () => ws.windows.flatMap((w) => w.sideTabs);
  const sideTab = (sid: string) => allSideTabs().find((s) => s.id === sid) ?? fail("UnknownSideTab", sid);
  const window = (wid: string) => ws.windows.find((w) => w.id === wid) ?? fail("UnknownWindow", wid);
  const holder = (tid: string) =>
    allSideTabs().find((s) => s.tabs.some((t) => t.id === tid)) ?? fail("UnknownTab", tid);
  const tabOfPane = (pid: string) =>
    allSideTabs()
      .flatMap((s) => s.tabs)
      .find((t) => panesOf(t.layout).includes(pid)) ?? fail("UnknownPane", pid);
  const inRange = (i: number, len: number) => (i >= 0 && i <= len ? i : fail("IndexOutOfRange", i));
  const closesSource = (tid: string, target?: string): Patch[] => {
    const s = holder(tid);
    return s.tabs.length === 1 && s.id !== target ? [{ type: "SideTabClosed", sideTabId: s.id }] : [];
  };

  function plan(c: Command): Patch[] {
    switch (c.type) {
      case "CreateSideTab": {
        const w = window(c.windowId);
        const index = inRange(c.index ?? w.sideTabs.length, w.sideTabs.length);
        return [{ type: "SideTabCreated", windowId: c.windowId, index, sideTab: newSideTab() }];
      }
      case "CreateTab": {
        const index = sideTab(c.sideTabId).tabs.length;
        return [{ type: "TabCreated", sideTabId: c.sideTabId, index, tab: newTab() }];
      }
      case "CloseTab": {
        const closed = closesSource(c.tabId);
        return [{ type: "TabClosed", tabId: c.tabId }, ...closed];
      }
      case "MoveTab": {
        const from = holder(c.tabId);
        const target = sideTab(c.toSideTabId);
        inRange(c.index, target.tabs.length - (from.id === target.id ? 1 : 0));
        const closed = closesSource(c.tabId, c.toSideTabId);
        return [{ type: "TabMoved", tabId: c.tabId, toSideTabId: c.toSideTabId, index: c.index }, ...closed];
      }
      case "MoveTabToNewSideTab": {
        holder(c.tabId);
        inRange(c.index, window(c.windowId).sideTabs.length);
        const closed = closesSource(c.tabId);
        const created: SideTab = { id: id(), title: null, tabs: [] };
        return [
          { type: "SideTabCreated", windowId: c.windowId, index: c.index, sideTab: created },
          { type: "TabMoved", tabId: c.tabId, toSideTabId: created.id, index: 0 },
          ...closed,
        ];
      }
      case "RenameTab": {
        holder(c.tabId);
        const title = c.title?.trim() || null;
        return [{ type: "TabRenamed", tabId: c.tabId, title }];
      }
      case "SplitPane":
      case "OpenTranscript": {
        if (c.type === "OpenTranscript" && !isSessionId(c.session)) return fail("BadSession", c.session);
        const tab = tabOfPane(c.paneId);
        const [dir, session] = c.type === "SplitPane" ? [c.dir, undefined] : (["row", c.session] as const);
        const layout = splitPane(tab.layout, c.paneId, dir, id(), id(), session);
        if (layout === "small") return fail("PaneTooSmall", c.paneId);
        return [{ type: "LayoutChanged", tabId: tab.id, layout: layout! }];
      }
      case "ClosePane": {
        const tab = tabOfPane(c.paneId);
        const layout = removePane(tab.layout, c.paneId);
        // A tab left with only transcript panes closes, as in the core.
        if (layout && terminalsOf(layout).length) return [{ type: "LayoutChanged", tabId: tab.id, layout }];
        return [{ type: "TabClosed", tabId: tab.id }, ...closesSource(tab.id)];
      }
      case "ResizeSplit": {
        const tab =
          allSideTabs()
            .flatMap((s) => s.tabs)
            .find((t) => hasSplit(t.layout, c.splitId)) ?? fail("UnknownSplit", c.splitId);
        const layout = resizeSplit(tab.layout, c.splitId, c.sizes);
        if (typeof layout === "string") return fail("BadSizes", c.splitId);
        return [{ type: "LayoutChanged", tabId: tab.id, layout }];
      }
      case "SetStatus": {
        const known = allSideTabs().some((s) => s.id === c.targetId || s.tabs.some((t) => t.id === c.targetId));
        if (!known) return fail("UnknownTab", c.targetId);
        if (c.status) {
          const err = lookError(c.status);
          if (err) return fail("BadStatus", err);
        }
        const status: ManualStatus | null = c.status && { ...c.status, label: c.status.label.trim() };
        return [{ type: "StatusSet", id: c.targetId, status }];
      }
      case "RestyleStatus": {
        const users = [...allSideTabs(), ...allSideTabs().flatMap((s) => s.tabs)].filter((x) => x.status?.statusId === c.statusId);
        return users.map((u) => ({ type: "StatusSet", id: u.id, status: c.status && { ...c.status, statusId: c.statusId } }));
      }
      case "ResumePane":
      case "DismissResume": {
        const tab = tabOfPane(c.paneId);
        const layout = structuredClone(tab.layout);
        const pane = leaf(layout, c.paneId)!;
        if (!pane.resume || !isSessionId(pane.resume)) return fail("NothingToResume", c.paneId);
        if (c.type === "ResumePane") typed.set(c.paneId, `claude --resume ${pane.resume}\r`);
        delete pane.resume;
        return [{ type: "LayoutChanged", tabId: tab.id, layout }];
      }
    }
  }

  const host: MockHost = {
    batches,
    openedUrls,
    resizes,
    typed,
    workspace: () => structuredClone(ws),
    async snapshot(): Promise<Snapshot> {
      return { rev, workspace: structuredClone(ws) };
    },
    async send(command) {
      const batch: PatchBatch = { rev: rev + 1, patches: plan(command) };
      ws = applyBatch(ws, batch);
      rev = batch.rev;
      batches.push(batch);
      for (const l of listeners) l(structuredClone(batch));
      recompute();
      return rev;
    },
    async onPatches(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    async openPane(paneId, cols, rows, onOutput): Promise<PaneView> {
      tabOfPane(paneId);
      const shell = shells.get(paneId) ?? new MockShell(paneId);
      shells.set(paneId, shell);
      shell.resize(cols, rows);
      let open = true;
      const emit = (s: string) => {
        if (open && s) queueMicrotask(() => open && onOutput(encoder.encode(s)));
      };
      emit(shell.redraw());
      return {
        input: (data) => emit(shell.input(data)),
        resize: (c, r) => {
          resizes.push(`${paneId} ${c}x${r}`);
          shell.resize(c, r);
        },
        ack: () => {},
        close: () => {
          open = false;
        },
      };
    },
    setAgent(pane, agent) {
      if (agent) agents.set(pane, agent);
      else agents.delete(pane);
      for (const l of agentListeners) l({ pane, agent });
      recompute();
    },
    runCommand(pane, command, exit) {
      // A rule status holds until its command runs again.
      const matching = config.rules.filter((r) => r.kind === "exitCode" && glob(r.pattern, command));
      for (const r of matching) dynamic.delete(dynKey(pane, r.id));
      shellFacts.set(pane, { running: exit === null, lastExit: exit, command });
      if (exit !== null && exit !== 0) {
        for (const r of matching.filter((r) => r.enabled)) {
          dynamic.set(dynKey(pane, r.id), { pane, look: r.look, source: "rule", order: ++order, ruleId: r.id });
        }
      }
      recompute();
    },
    setCliStatus(pane, look) {
      if (look) dynamic.set(dynKey(pane, "cli"), { pane, look: checked(look), source: "cli", order: ++order });
      else dynamic.delete(dynKey(pane, "cli"));
      recompute();
    },
    async statuses() {
      recompute();
      return structuredClone(shown);
    },
    async onStatuses(listener) {
      statusListeners.add(listener);
      return () => statusListeners.delete(listener);
    },
    async statusConfig() {
      return structuredClone(config);
    },
    async onStatusConfig(listener) {
      configListeners.add(listener);
      return () => configListeners.delete(listener);
    },
    async saveStatus(sideTabId, look) {
      const st: SavedStatus = { id: id(), sideTabId, ...checked(look) };
      config.statuses.push(st);
      configChanged();
      return structuredClone(st);
    },
    async updateStatus(sid, look) {
      const st = Object.assign(savedStatus(sid), checked(look));
      configChanged();
      await host.send({ type: "RestyleStatus", statusId: sid, status: { ...checked(look), statusId: sid } });
      return structuredClone(st);
    },
    async deleteStatus(sid) {
      config.statuses = config.statuses.filter((x) => x.id !== sid);
      configChanged();
      await host.send({ type: "RestyleStatus", statusId: sid, status: null });
    },
    async makeGlobal(sid) {
      const st = savedStatus(sid);
      st.sideTabId = null;
      // A promoted status goes last among the globals.
      config.statuses = [...config.statuses.filter((x) => x.sideTabId === null && x !== st), st, ...config.statuses.filter((x) => x.sideTabId !== null)];
      configChanged();
      return structuredClone(st);
    },
    async reorderStatuses(ids) {
      const at = (x: SavedStatus) => (ids.includes(x.id) ? ids.indexOf(x.id) : ids.length);
      config.statuses = [...config.statuses].sort((a, b) => at(a) - at(b));
      configChanged();
    },
    async addRule(statusId, kind) {
      const st = savedStatus(statusId);
      if (kind.kind === "exitCode" && !kind.pattern.trim()) throw new Error("a rule needs a command pattern");
      if (kind.kind === "claudeIdle" && !(kind.minutes >= 1 && kind.minutes <= 1440)) throw new Error("an idle rule waits 1 to 1440 minutes");
      const rule = {
        id: id(),
        statusId,
        look: { label: st.label, accent: st.accent, background: st.background },
        enabled: true,
        ...(kind.kind === "exitCode" ? { kind: "exitCode", pattern: kind.pattern.trim() } : kind),
      } as Rule;
      config.rules.push(rule);
      configChanged();
      return structuredClone(rule);
    },
    async toggleRule(rid, enabled) {
      config.rules = config.rules.map((r) => (r.id === rid ? { ...r, enabled } : r));
      configChanged();
    },
    async deleteRule(rid) {
      config.rules = config.rules.filter((r) => r.id !== rid);
      configChanged();
    },
    async setBuiltin(auto, look) {
      const builtins = { ...config.builtins };
      if (look) {
        if (auto === "waiting" && (look.accent || look.background)) throw new Error("Waiting stays amber");
        builtins[auto] = checked(look);
      } else delete builtins[auto];
      config = { ...config, builtins };
      configChanged();
    },
    async agents() {
      return [...agents].map(([pane, agent]) => ({ pane, agent }));
    },
    async onAgent(listener) {
      agentListeners.add(listener);
      return () => agentListeners.delete(listener);
    },
    appendTranscript(session, entries) {
      const list = transcripts.get(session) ?? [];
      // Offsets stand in for byte positions: one per entry.
      for (const e of entries) list.push({ ...e, offset: list.length });
      transcripts.set(session, list);
    },
    async readTranscript(session, offset) {
      const list = transcripts.get(session);
      if (!list) throw new Error(`no transcript for session ${session}`);
      return { entries: structuredClone(list.slice(offset)), offset: list.length, reset: false };
    },
    async pins(side) {
      return structuredClone(pinList.filter((p) => p.sideTabId === side));
    },
    async addPin(pin) {
      if (!isSessionId(pin.session)) throw new Error(`${pin.session} is not a Claude session id`);
      const saved: Pin = { ...pin, id: id(), createdAt: Date.now() };
      pinList.push(saved);
      contextChanged(pin.sideTabId);
      return structuredClone(saved);
    },
    async removePin(pinId) {
      const at = pinList.findIndex((p) => p.id === pinId);
      if (at < 0) return;
      const [gone] = pinList.splice(at, 1);
      contextChanged(gone!.sideTabId);
    },
    async variables(side) {
      return structuredClone(vars.filter((v) => v.sideTabId === side));
    },
    async extractVariable(side, name, value) {
      if (!/^[A-Za-z_][A-Za-z0-9_]{0,63}$/.test(name) || name.startsWith("TERMINESCU_")) {
        throw new Error(`${name} is not a variable name`);
      }
      if (vars.some((v) => v.sideTabId === side && v.name === name)) {
        throw new Error(`side tab ${side} already has a variable ${name}`);
      }
      const v: Variable = { id: id(), sideTabId: side, name, value, source: "extracted" };
      vars.push(v);
      contextChanged(side);
      return structuredClone(v);
    },
    async onContext(listener) {
      contextListeners.add(listener);
      return () => contextListeners.delete(listener);
    },
    async openUrl(url) {
      openedUrls.push(url);
    },
    // Outside Tauri there is no app menu: chords come from key events, caught before xterm.js.
    async onAction(listener) {
      const target = globalThis as Partial<typeof globalThis>;
      if (!target.addEventListener || !target.removeEventListener) return () => {};
      const onKey = (e: KeyboardEvent) => {
        const action = actionFor(e);
        if (!action) return;
        e.preventDefault();
        e.stopPropagation();
        listener(action);
      };
      target.addEventListener("keydown", onKey, true);
      return () => target.removeEventListener?.("keydown", onKey, true);
    },
  };
  return host;
}

// A Host without Tauri, for unit tests and Playwright runs of the Vite build (task 1.3). It
// plans commands like crates/core/src/state.rs, with readable ids (m1, m2, ...).

import type { Host, PaneView } from "./host";
import { ACTIONS, actionFor, bindingProblem, normalAccel, withBindings, type KeyBindings } from "./keys";
import { PromptStore } from "./prompt-store";
import { hasSplit, holdsWork, isSessionId, leaf, panesOf, removePane, resizeSplit, splitPane, terminalsOf } from "./layout";
import {
  ACCENTS,
  type DragEnd,
  type DragPoint,
  MAX_LABEL,
  DEFAULT_SETTINGS,
  MAX_TTL_SECS,
  ZOOM_STEPS,
  type AppSettings,
  TINTS,
  type AgentUpdate,
  type AgentView,
  type AttentionSettings,
  type SavedCommand,
  type Command,
  type Entry,
  type Id,
  type Look,
  type ManualStatus,
  type Pin,
  type Variable,
  type Item,
  type TouchedFile,
  type NewVariable,
  isVariableName,
  MERGE_UNDO_MS,
  suggestRename,
  type MergeClash,
  type CommandError,
  type DocView,
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
import { applyLaunchers, computeStatuses, type PaneDynamic, type PaneFacts } from "./status";

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
  /** Every byte a pane view sent as input, per pane, keys and inserts alike. */
  readonly inputs: Map<string, string>;
  /** How many times the chime was asked for. */
  chimes(): number;
  /** Plays the core's part for hook events: sets or clears a pane's agent. */
  setAgent(pane: string, agent: AgentView | null): void;
  /** Appends entries to a session's fake transcript. */
  appendTranscript(session: string, entries: Entry[]): void;
  /** Where the next tab drag lands, as the core would say from the cursor; once. */
  setDragOutcome(kind: DragEnd["kind"]): void;
  /** Plays a drag from another window: over this one at a point, or gone (`null`). */
  dragHover(point: DragPoint | null): void;
  /** Plays a drop from another window at a point. */
  dragDrop(point: DragPoint): void;
  /** Tab drags begun, as `tabId title`. */
  readonly dragsBegun: string[];
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
  /** The mock's disk: absolute path to text. A missing path is a missing file. */
  readonly files: Map<string, string>;
  /** Answers for the next Open or Save dialogs, in order; null cancels. */
  readonly dialogAnswers: (string | null)[];
  /** Each dialog the UI opened, as `open` or `save`. */
  readonly dialogs: string[];
  /** Plays someone changing a file on disk: a clean buffer reloads, a dirty one gets the bar. */
  writeFile(path: string, text: string): void;
  /**
   * Plays `terminescu edit --wait path` in `pane`: the file opens beside it and the promise
   * settles when the document returns or its pane closes.
   */
  editWait(pane: Id, path: string): Promise<void>;
  /** Plays files dropped from Finder. */
  dropFiles(paths: string[]): void;
  /** Plays a `PostToolUse` hook for Write or Edit in a pane: the file joins its side tab's Files. */
  touchFile(pane: Id, path: string, change: "added" | "modified"): void;
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
  const files = new Map<string, string>();
  const dialogAnswers: (string | null)[] = [];
  const dialogs: string[] = [];
  // Documents as in crates/core/src/documents.rs, without hot exit.
  const docs = new Map<string, { view: DocView; text: string; saved: string }>();
  const docListeners = new Set<(d: DocView) => void>();
  const full = (d: { view: DocView; text: string }): DocView => ({ ...structuredClone(d.view), text: d.text });
  const meta = (d: { view: DocView }): DocView => structuredClone(d.view);
  const docOrFail = (docId: string) => {
    const d = docs.get(docId);
    if (!d) throw new Error(`no document ${docId}`);
    return d;
  };
  const baseName = (path: string) => path.split("/").pop() || path;
  const newDoc = (source: DocView["source"], title: string, text: string, missing = false) => {
    const view: DocView = {
      id: id(),
      source,
      title,
      dirty: false,
      conflict: false,
      readOnly: false,
      bom: false,
      missing,
      sessionOnly: false,
      request: false,
      rev: 0,
    };
    docs.set(view.id, { view, text, saved: text });
    return view.id;
  };
  const showDoc = async (sideTabId: string, docId: string, beside?: string | null) => {
    try {
      await host.send({ type: "OpenText", sideTabId, docId, beside: beside ?? null });
    } catch (e) {
      docs.delete(docId);
      throw e;
    }
    return full(docs.get(docId)!);
  };
  // `terminescu edit --wait` calls by document, as `Waits` in the app.
  const waits = new Map<string, (() => void)[]>();
  const endWaits = (docId: string) => {
    for (const done of waits.get(docId) ?? []) done();
    waits.delete(docId);
  };
  const dropListeners = new Set<(paths: string[]) => void>();
  const MOCK_CWD = "/Users/owner/acme-api";
  const inputs = new Map<string, string>();
  const agents = new Map<string, AgentView>();
  const agentListeners = new Set<(u: AgentUpdate) => void>();
  // Waiting panes and when they started, as crates/core/src/attention.rs keeps them.
  const waitingSince = new Map<string, number>();
  let waitClock = 0;
  let attention: AttentionSettings = { notify: true, chime: false };
  let settings: AppSettings = { ...DEFAULT_SETTINGS };
  const settingsListeners = new Set<() => void>();
  let keyBindings: KeyBindings = {};
  // As the app menu while Settings › Keys records: no action chords.
  let recording = false;
  const keyListeners = new Set<() => void>();
  let chimes = 0;
  const revealListeners = new Set<(pane: Id) => void>();
  let savedCommands: SavedCommand[] = [];
  const commandListeners = new Set<() => void>();
  const commandsChanged = () => commandListeners.forEach((l) => l());
  const checkCommand = (name: string, body: string) => {
    const clean = body.replace(/[\r\n]+$/, "");
    if (!name.trim() || name.trim().length > 80) throw new Error("a command needs a name of at most 80 characters");
    if (!clean.trim()) throw new Error("the command is empty");
    return { name: name.trim(), body: clean };
  };
  // As crates/core/src/commands.rs: a secret becomes $NAME, a missing name stays.
  const resolveCommand = (body: string, side: string, extra: Record<string, string>) => {
    const missing: string[] = [];
    const unsafe: string[] = [];
    const text = body.replace(/\{\{([^{}]*)\}\}/g, (whole, raw: string) => {
      const name = raw.trim();
      const v = live(side).find((x) => x.name === name);
      const value = v ? (v.mode === "secret" ? `$${v.name}` : (v.value ?? "")) : extra[name] || undefined;
      if (value === undefined) {
        if (isVariableName(name) && !missing.includes(name)) missing.push(name);
        return whole;
      }
      if (/[\x00-\x1f\x7f]/.test(value) && !unsafe.includes(name)) unsafe.push(name);
      return value;
    });
    return { text, missing, unsafe };
  };
  const typeBytes = (text: string, run: boolean) => {
    // As `typeable` in the core: every control but \n and \t goes, ESC included.
    let t = text;
    for (let prev = ""; prev !== t; ) [prev, t] = [t, t.replaceAll("\x1b[200~", "").replaceAll("\x1b[201~", "")];
    const clean = t.replace(/\r\n?/g, "\n").replace(/[\x00-\x08\x0b-\x1f\x7f-\x9f]/g, "");
    return (clean.includes("\n") ? `\x1b[200~${clean}\x1b[201~` : clean) + (run ? "\r" : "");
  };
  const typeInto = (pane: string, bytes: string) => typed.set(pane, (typed.get(pane) ?? "") + bytes);
  // As `launch` in the app: its own tab when that tab still exists, else a new one. Returns an
  // error text, or undefined once typed.
  const launch = async (c: SavedCommand): Promise<string | undefined> => {
    const { text, missing, unsafe } = resolveCommand(c.body, c.sideTabId!, {});
    if (missing.length) return `${c.name} needs a value for ${missing.join(", ")} in its side tab first`;
    if (unsafe.length) return `${unsafe.join(", ")} holds a line break, so ${c.name} cannot run on its own`;
    const tab = ws.windows.flatMap((w) => w.sideTabs.flatMap((s) => s.tabs)).find((t) => t.id === c.tabId);
    const pane = tab && terminalsOf(tab.layout)[0];
    if (pane) {
      typeInto(pane, typeBytes(text, true));
      return undefined;
    }
    const before = new Set(terminalPanes());
    await host.send({ type: "CreateTab", sideTabId: c.sideTabId! });
    const fresh = terminalPanes().find((p) => !before.has(p));
    if (fresh) typeInto(fresh, typeBytes(text, true));
    c.tabId = fresh ? tabOfPane(fresh).id : null;
    return undefined;
  };
  const terminalPanes = () => ws.windows.flatMap((w) => w.sideTabs.flatMap((s) => s.tabs.flatMap((t) => terminalsOf(t.layout))));
  const chimeListeners = new Set<() => void>();
  const transcripts = new Map<string, Entry[]>();
  const pinList: Pin[] = [];
  const vars: Variable[] = [];
  // Secret values stay here and never come back out through `variables` (D21).
  const secrets = new Map<string, string>();
  const secretKey = (side: string, name: string) => `${side}\u0000${name}`;
  const itemList: Item[] = [];
  const touched: TouchedFile[] = [];
  let pastes = 1;
  const live = (side: string) => {
    const now = Date.now();
    for (let i = vars.length - 1; i >= 0; i--) {
      const v = vars[i]!;
      if (v.expiresAt != null && v.expiresAt <= now) {
        vars.splice(i, 1);
        secrets.delete(secretKey(v.sideTabId, v.name));
      }
    }
    return vars.filter((v) => v.sideTabId === side);
  };
  const putVariable = (side: string, nv: NewVariable, replace: boolean): Variable => {
    if (!isVariableName(nv.name)) throw new Error(`${nv.name} is not a variable name`);
    if (!nv.value.trim()) throw new Error("nothing to save");
    const at = live(side).findIndex((v) => v.name === nv.name);
    const old = at >= 0 ? live(side)[at] : undefined;
    if (old && !replace) throw new Error(`this side tab already has a variable ${nv.name}`);
    if (old?.mode === "secret" && nv.mode !== "secret") {
      throw new Error(`${nv.name} is a secret and stays one; remove it and add it again to change its mode`);
    }
    if (old) vars.splice(vars.indexOf(old), 1);
    const v: Variable = {
      id: old?.mode === "saved" && nv.mode === "saved" ? old.id : nv.mode === "saved" ? id() : `${nv.mode}:${nv.name}`,
      sideTabId: side,
      name: nv.name,
      value: nv.mode === "secret" ? null : nv.value,
      mode: nv.mode,
      source: nv.source,
      originTabId: nv.originTabId ?? null,
      expiresAt: nv.mode !== "saved" && nv.ttlSecs ? Date.now() + nv.ttlSecs * 1000 : null,
    };
    if (nv.mode === "secret") secrets.set(secretKey(side, nv.name), nv.value);
    else secrets.delete(secretKey(side, nv.name));
    vars.push(v);
    contextChanged(side);
    return structuredClone(v);
  };
  const contextListeners = new Set<(sideTabId: string) => void>();
  const promptListeners = new Set<() => void>();
  // Invented seeds, enough for the library and insert tests.
  const library = new PromptStore(() => queueMicrotask(() => promptListeners.forEach((l) => l())));
  library.create({ key: "review-plan", name: "Review plan", tags: ["plan", "review"], body: "Review the plan for {{ticket}}. Rate your confidence from 1 to 10.\n" });
  library.create({ key: "open-prs", name: "Open PRs", tags: ["git"], body: "Open a PR from {{branch}} into develop.\n" });
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
    const launchers = new Map<string, (string | null)[]>();
    for (const c of savedCommands.filter((c) => c.launcher && c.sideTabId)) {
      launchers.set(c.sideTabId!, [...(launchers.get(c.sideTabId!) ?? []), c.tabId]);
    }
    applyLaunchers(next, launchers);
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

  // One batch through the reducer to every window, as the core's `handle` and emit.
  // Task 8.1: the drag the core would track, and what tests make it say.
  let dragging: { tabId: Id; token: number } | undefined;
  let dragToken = 0;
  let dragOutcome: DragEnd["kind"] = "local";
  const dragsBegun: string[] = [];
  const hoverListeners = new Set<(p: DragPoint | null) => void>();
  const tabDropListeners = new Set<(p: DragPoint) => void>();
  const commit = async (planned: Patch[]): Promise<number> => {
    // A window the batch emptied closes, unless it is the last one, as in the core.
    const had = new Set(ws.windows.filter((w) => w.sideTabs.length > 0).map((w) => w.id));
    const after = applyBatch(ws, { rev: rev + 1, patches: planned });
    const closes: Patch[] = [];
    for (const w of after.windows)
      if (w.sideTabs.length === 0 && had.has(w.id) && after.windows.length - closes.length > 1) closes.push({ type: "WindowClosed", windowId: w.id });
    const batch: PatchBatch = { rev: rev + 1, patches: [...planned, ...closes] };
    ws = applyBatch(ws, batch);
    rev = batch.rev;
    batches.push(batch);
    for (const l of listeners) l(structuredClone(batch));
    recompute();
    // A document goes with its last pane, as `on_docs_closed` in the runtime.
    const shown = new Set(
      ws.windows.flatMap((w) => w.sideTabs.flatMap((s) => s.tabs.flatMap((t) => panesOf(t.layout).map((p) => leaf(t.layout, p)!.doc)))),
    );
    for (const d of [...docs.keys()]) {
      if (shown.has(d)) continue;
      docs.delete(d);
      endWaits(d);
    }
    return rev;
  };
  // Merges inside their undo window (D5): a snapshot of the context lists to put back.
  const merges = new Map<string, { restore(): void; windowId: Id; index: number; side: SideTab; tabIds: Id[]; into: Id; at: number }>();
  const clashesOf = (from: Id, into: Id, label: string | null): MergeClash[] => {
    const a = live(from);
    const b = live(into);
    const taken = new Set([...a, ...b].map((v) => v.name));
    const out: MergeClash[] = [];
    for (const v of a) {
      const t = b.find((x) => x.name === v.name);
      if (!t || (v.mode !== "secret" && t.mode !== "secret" && v.value === t.value)) continue;
      const rename = suggestRename(v.name, label, taken);
      taken.add(rename);
      out.push({ name: v.name, fromValue: v.value, fromMode: v.mode, intoValue: t.value, intoMode: t.mode, rename });
    }
    return out;
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
      case "MoveTabToNewWindow": {
        holder(c.tabId);
        const closed = closesSource(c.tabId);
        const windowId = id();
        const created: SideTab = { id: id(), title: null, tabs: [] };
        return [
          { type: "WindowCreated", windowId },
          { type: "SideTabCreated", windowId, index: 0, sideTab: created },
          { type: "TabMoved", tabId: c.tabId, toSideTabId: created.id, index: 0 },
          ...closed,
        ];
      }
      case "RenameSideTab": {
        sideTab(c.sideTabId);
        return [{ type: "SideTabRenamed", sideTabId: c.sideTabId, title: c.title?.trim() || null }];
      }
      case "CloseSideTab":
        sideTab(c.sideTabId);
        return [{ type: "SideTabClosed", sideTabId: c.sideTabId }];
      case "MoveSideTabToNewWindow": {
        const moved = structuredClone(sideTab(c.sideTabId));
        const windowId = id();
        return [
          { type: "WindowCreated", windowId },
          { type: "SideTabClosed", sideTabId: c.sideTabId },
          { type: "SideTabCreated", windowId, index: 0, sideTab: moved },
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
        const layout = splitPane(tab.layout, c.paneId, dir, id(), id(), session ? { transcript: session } : {});
        if (layout === "small") return fail("PaneTooSmall", c.paneId);
        return [{ type: "LayoutChanged", tabId: tab.id, layout: layout! }];
      }
      case "OpenText": {
        const side = sideTab(c.sideTabId);
        if (c.beside) {
          const tab = side.tabs.find((t) => panesOf(t.layout).includes(c.beside!)) ?? fail("UnknownPane", c.beside);
          const layout = splitPane(tab.layout, c.beside, "row", id(), id(), { doc: c.docId });
          if (layout === "small") return fail("PaneTooSmall", c.beside);
          return [{ type: "LayoutChanged", tabId: tab.id, layout: layout! }];
        }
        const tab: Tab = { id: id(), title: null, layout: { type: "pane", id: id(), doc: c.docId } };
        return [{ type: "TabCreated", sideTabId: c.sideTabId, index: side.tabs.length, tab }];
      }
      case "ClosePane": {
        const tab = tabOfPane(c.paneId);
        const layout = removePane(tab.layout, c.paneId);
        // A tab left with only transcript panes closes, as in the core.
        if (layout && holdsWork(layout)) return [{ type: "LayoutChanged", tabId: tab.id, layout }];
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
    inputs,
    workspace: () => structuredClone(ws),
    async snapshot(): Promise<Snapshot> {
      return { rev, workspace: structuredClone(ws) };
    },
    async send(command) {
      return commit(plan(command));
    },
    async mergePreview(from, into, label) {
      sideTab(from);
      sideTab(into);
      return clashesOf(from, into, label);
    },
    async mergeSideTabs(from, into, label, resolution) {
      if (from === into) fail("UnknownSideTab", from);
      const w = ws.windows.find((x) => x.sideTabs.some((s) => s.id === from)) ?? fail("UnknownSideTab", from);
      const target = sideTab(into);
      const index = w.sideTabs.findIndex((s) => s.id === from);
      const source = w.sideTabs[index]!;
      const clashes = clashesOf(from, into, label);
      const taken = new Set([...live(from), ...live(into)].map((v) => v.name));
      const rename = new Map<string, string>();
      if (resolution.keepBoth) {
        for (const c of clashes) {
          const to = resolution.renames?.[c.name] ?? c.rename;
          if (!isVariableName(to)) throw new Error(`${to} is not a variable name`);
          if (taken.has(to)) throw new Error(`this side tab already has a variable ${to}`);
          taken.add(to);
          rename.set(c.name, to);
        }
      }
      const before = {
        vars: structuredClone(vars),
        secrets: new Map(secrets),
        items: structuredClone(itemList),
        pins: structuredClone(pinList),
        touched: structuredClone(touched),
        commands: structuredClone(savedCommands),
      };
      // Moved entries go to the end, so the target's come first.
      const toEnd = <T>(list: T[], x: T, keep: boolean) => {
        list.splice(list.indexOf(x), 1);
        if (keep) list.push(x);
      };
      const has = new Set(live(into).map((v) => v.name));
      for (const v of live(from)) {
        const to = has.has(v.name) ? rename.get(v.name) : v.name;
        const secret = secrets.get(secretKey(from, v.name));
        secrets.delete(secretKey(from, v.name));
        toEnd(vars, v, !!to);
        if (!to) continue;
        if (secret !== undefined) secrets.set(secretKey(into, to), secret);
        v.name = to;
        v.sideTabId = into;
        v.id = v.mode === "saved" ? v.id : `${v.mode}:${to}`;
      }
      const links = new Set(itemList.filter((i) => i.sideTabId === into && i.kind === "link").map((i) => i.value));
      for (const i of itemList.filter((x) => x.sideTabId === from)) {
        toEnd(itemList, i, !(i.kind === "link" && links.has(i.value)));
        i.sideTabId = into;
      }
      for (const p of pinList) if (p.sideTabId === from) p.sideTabId = into;
      const paths = new Set(touched.filter((f) => f.sideTabId === into).map((f) => f.path));
      for (const f of touched.filter((x) => x.sideTabId === from)) {
        if (paths.has(f.path)) touched.splice(touched.indexOf(f), 1);
        else f.sideTabId = into;
      }
      for (const c of savedCommands) if (c.sideTabId === from) c.sideTabId = into;
      const undoId = id();
      merges.set(undoId, {
        restore: () => {
          vars.splice(0, vars.length, ...before.vars);
          secrets.clear();
          for (const [k, v] of before.secrets) secrets.set(k, v);
          itemList.splice(0, itemList.length, ...before.items);
          pinList.splice(0, pinList.length, ...before.pins);
          touched.splice(0, touched.length, ...before.touched);
          savedCommands = before.commands;
        },
        windowId: w.id,
        index,
        side: { ...structuredClone(source), tabs: [] },
        tabIds: source.tabs.map((t) => t.id),
        into,
        at: Date.now(),
      });
      const len = target.tabs.length;
      await commit([
        ...source.tabs.map((t, i): Patch => ({ type: "TabMoved", tabId: t.id, toSideTabId: into, index: len + i })),
        { type: "SideTabClosed", sideTabId: from },
      ]);
      contextChanged(into);
      commandsChanged();
      return { undoId, renames: [...rename] };
    },
    async dragBegin(tabId, title) {
      dragging = { tabId, token: ++dragToken };
      dragsBegun.push(`${tabId} ${title}`);
      return dragToken;
    },
    async dragEnd(token, drop) {
      const tabId = dragging?.token === token ? dragging.tabId : undefined;
      if (tabId) dragging = undefined;
      const kind = dragOutcome;
      dragOutcome = "local";
      if (!tabId || !drop) return { kind: "cancelled" };
      if (kind === "newWindow") await host.send({ type: "MoveTabToNewWindow", tabId });
      return { kind };
    },
    async onDragHover(listener) {
      hoverListeners.add(listener);
      return () => hoverListeners.delete(listener);
    },
    async onDragDrop(listener) {
      tabDropListeners.add(listener);
      return () => tabDropListeners.delete(listener);
    },
    setDragOutcome(kind) {
      dragOutcome = kind;
    },
    dragHover(point) {
      for (const l of hoverListeners) l(point);
    },
    dragDrop(point) {
      for (const l of tabDropListeners) l(point);
    },
    dragsBegun,
    async undoMerge(undoId) {
      const m = merges.get(undoId);
      merges.delete(undoId);
      if (!m || Date.now() - m.at > MERGE_UNDO_MS + 2000) throw new Error("too late to undo this merge");
      const tabIds = m.tabIds.filter((t) => sideTab(m.into).tabs.some((x) => x.id === t));
      if (!tabIds.length || tabIds.length === sideTab(m.into).tabs.length) throw new Error("the merge cannot be undone");
      m.restore();
      const w = window(m.windowId);
      await commit([
        { type: "SideTabCreated", windowId: m.windowId, index: Math.min(m.index, w.sideTabs.length), sideTab: m.side },
        ...tabIds.map((t, i): Patch => ({ type: "TabMoved", tabId: t, toSideTabId: m.side.id, index: i })),
      ]);
      contextChanged(m.into);
      contextChanged(m.side.id);
      commandsChanged();
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
        input: (data) => {
          inputs.set(paneId, (inputs.get(paneId) ?? "") + data);
          emit(shell.input(data));
        },
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
      if (agent?.state !== "waiting") waitingSince.delete(pane);
      else if (!waitingSince.has(pane)) {
        waitingSince.set(pane, ++waitClock);
        if (attention.chime) {
          chimes++;
          for (const l of chimeListeners) l();
        }
      }
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
    async commands(side) {
      const mine = savedCommands.filter((c) => side !== null && c.sideTabId === side);
      const global = savedCommands.filter((c) => c.sideTabId === null);
      const order = (a: SavedCommand, b: SavedCommand) => a.position - b.position;
      return structuredClone([...mine.sort(order), ...global.sort(order)]);
    },
    async createCommand(c) {
      const { name, body } = checkCommand(c.name, c.body);
      const position = savedCommands.filter((x) => x.sideTabId === c.sideTabId).length;
      if (c.launcher && c.sideTabId === null) throw new Error("a launcher command belongs to one side tab");
      const saved: SavedCommand = { ...c, name, body, id: id(), tabId: null, position, createdAt: Date.now() };
      savedCommands.push(saved);
      const started = saved.launcher ? await launch(saved) : undefined;
      commandsChanged();
      recompute();
      if (started) throw new Error(`saved, but not started: ${started}`);
      return structuredClone(saved);
    },
    async updateCommand(cid, c) {
      const old = savedCommands.find((x) => x.id === cid);
      if (!old) throw new Error(`no command ${cid}`);
      const { name, body } = checkCommand(c.name, c.body);
      if (c.launcher && old.sideTabId === null) throw new Error("a launcher command belongs to one side tab");
      const wasLauncher = old.launcher;
      Object.assign(old, { name, body, target: c.target, launcher: c.launcher });
      const started = old.launcher && !wasLauncher ? await launch(old) : undefined;
      commandsChanged();
      recompute();
      if (started) throw new Error(`saved, but not started: ${started}`);
      return structuredClone(old);
    },
    async deleteCommand(cid) {
      savedCommands = savedCommands.filter((x) => x.id !== cid);
      commandsChanged();
      recompute();
    },
    async runSavedCommand(r) {
      const c = savedCommands.find((x) => x.id === r.id);
      if (!c) throw new Error(`no command ${r.id}`);
      if (c.sideTabId !== null && c.sideTabId !== r.sideTabId) throw new Error("this command belongs to another side tab");
      const { text, missing, unsafe } = resolveCommand(c.body, r.sideTabId, r.extra ?? {});
      if (missing.length) return { kind: "missing", names: missing };
      if (r.run && unsafe.length) return { kind: "unsafe", names: unsafe };
      const bytes = typeBytes(text, r.run);
      const target = r.target ?? c.target;
      if (target === "focused") {
        if (!r.paneId) throw new Error("focus a terminal pane first");
        if (holder(tabOfPane(r.paneId).id).id !== r.sideTabId) throw new Error("the focused pane is in another side tab");
        typeInto(r.paneId, bytes);
        return { kind: "done", pane: r.paneId };
      }
      const before = new Set(terminalPanes());
      await host.send(target === "newTab" ? { type: "CreateTab", sideTabId: r.sideTabId } : { type: "CreateSideTab", windowId: r.windowId });
      const pane = terminalPanes().find((p) => !before.has(p)) ?? null;
      if (pane) typeInto(pane, bytes);
      return { kind: "done", pane };
    },
    async restartLauncher(side, cid) {
      const errors: string[] = [];
      for (const c of savedCommands.filter((c) => c.launcher && c.sideTabId === side && (!cid || c.id === cid))) {
        const up = c.tabId !== null && shown[c.tabId]?.auto === "running";
        if (up) continue;
        const e = await launch(c);
        if (e) errors.push(e);
      }
      commandsChanged();
      recompute();
      if (errors.length) throw new Error(errors.join("; "));
    },
    async onCommands(listener) {
      commandListeners.add(listener);
      return () => commandListeners.delete(listener);
    },
    async jumpWaiting() {
      const live = new Set(ws.windows.flatMap((w) => w.sideTabs.flatMap((s) => s.tabs.flatMap((t) => panesOf(t.layout)))));
      for (const p of waitingSince.keys()) if (!live.has(p)) waitingSince.delete(p);
      let oldest: Id | null = null;
      for (const [p, at] of waitingSince) if (oldest === null || at < waitingSince.get(oldest)!) oldest = p;
      if (oldest !== null) for (const l of revealListeners) l(oldest);
      return oldest;
    },
    async onRevealPane(listener) {
      revealListeners.add(listener);
      return () => revealListeners.delete(listener);
    },
    async onChime(listener) {
      chimeListeners.add(listener);
      return () => chimeListeners.delete(listener);
    },
    async allCommands() {
      const order = (a: SavedCommand, b: SavedCommand) =>
        Number(a.sideTabId !== null) - Number(b.sideTabId !== null) ||
        (a.sideTabId ?? "").localeCompare(b.sideTabId ?? "") ||
        a.position - b.position;
      return structuredClone([...savedCommands].sort(order));
    },
    async settings() {
      return { ...settings };
    },
    async setSettings(next) {
      const ttl = next.varTtlSecs;
      if (ttl !== null && (!Number.isInteger(ttl) || ttl <= 0 || ttl > MAX_TTL_SECS)) throw new Error("a TTL runs from 1 second to 30 days");
      const zoom = next.zoomPercent;
      if (!Number.isInteger(zoom) || zoom < ZOOM_STEPS[0]! || zoom > ZOOM_STEPS.at(-1)!) throw new Error("the zoom runs from 50% to 200%");
      settings = { ...next };
      // The app zooms its webview natively; a browser has CSS zoom on the root.
      if (typeof document !== "undefined") document.documentElement.style.zoom = zoom === 100 ? "" : String(zoom / 100);
      settingsListeners.forEach((l) => l());
      return { ...settings };
    },
    async keyBindings() {
      return { ...keyBindings };
    },
    // As keys::set in the core: checks every chord, drops the ones equal to the default.
    async setKeyBindings(next) {
      const kept: KeyBindings = {};
      for (const [id, accel] of Object.entries(next)) {
        const a = ACTIONS.find((x) => x.id === id);
        if (!a) throw new Error(`no action ${id}`);
        if (normalAccel(accel) !== normalAccel(a.accel)) kept[id] = accel;
      }
      const actions = withBindings(ACTIONS, kept);
      for (const a of actions) {
        const problem = bindingProblem(a.id, a.accel, actions);
        if (problem) throw new Error(problem);
      }
      keyBindings = kept;
      keyListeners.forEach((l) => l());
      return { ...kept };
    },
    async recordKeys(on) {
      recording = on;
    },
    async onKeyBindings(listener) {
      keyListeners.add(listener);
      return () => keyListeners.delete(listener);
    },
    async onSettings(listener) {
      settingsListeners.add(listener);
      return () => settingsListeners.delete(listener);
    },
    async attentionSettings() {
      return { ...attention };
    },
    async setAttentionSettings(settings) {
      attention = { ...settings };
    },
    chimes: () => chimes,
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
      const transcript = pin.session != null && pin.entryId != null && pin.paneId == null;
      const terminal = pin.session == null && pin.paneId != null && pin.rowStart != null && pin.rowEnd != null;
      if (!transcript && !terminal) throw new Error("a pin needs a session and entry, or a pane and rows");
      if (transcript && !isSessionId(pin.session!)) throw new Error(`${pin.session} is not a Claude session id`);
      if (!pin.exact.trim()) throw new Error("nothing to save");
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
      const order = (v: Variable) => (v.mode === "saved" ? 0 : 1);
      return structuredClone(
        live(side)
          .map((v, i) => [v, i] as const)
          .sort(([a, i], [b, j]) => order(a) - order(b) || (order(a) ? a.name.localeCompare(b.name) : i - j))
          .map(([v]) => v),
      );
    },
    async addVariable(side, nv, replace = false) {
      return putVariable(side, nv, replace);
    },
    async changeVariable(side, name, change) {
      const old = live(side).find((v) => v.name === name);
      if (!old) throw new Error(`no variable ${name}`);
      const mode = change.mode ?? old.mode;
      if (old.mode === "secret" && mode !== "secret") {
        throw new Error(`${name} is a secret and stays one; remove it and add it again to change its mode`);
      }
      // As in the core: the daemon never hands a secret back, so a new TTL needs the value again.
      if (change.value == null && old.value == null) {
        if (change.ttlSecs == null) return structuredClone(old);
        throw new Error(`type the value of ${name} again to change its TTL`);
      }
      const value = (change.value ?? old.value)!;
      const ttlSecs = change.ttlSecs ?? (old.expiresAt ? Math.max(1, Math.round((old.expiresAt - Date.now()) / 1000)) : null);
      return putVariable(side, { name, value, mode, ttlSecs, source: old.source ?? "typed", originTabId: old.originTabId }, true);
    },
    async removeVariable(side, name) {
      const at = vars.findIndex((v) => v.sideTabId === side && v.name === name);
      if (at < 0) return;
      vars.splice(at, 1);
      secrets.delete(secretKey(side, name));
      contextChanged(side);
    },
    async items(side) {
      return structuredClone(itemList.filter((i) => i.sideTabId === side));
    },
    async addItem(side, kind, raw, source, originTabId = null) {
      const value = raw.trim();
      if (!value) throw new Error("nothing to save");
      if (kind === "link" && (!/^https?:\/\//.test(value) || /\s/.test(value))) {
        throw new Error(`${value.slice(0, 80)} is not an http(s) link`);
      }
      const same = itemList.find((i) => i.sideTabId === side && i.kind === "link" && kind === "link" && i.value === value);
      if (same) return structuredClone(same);
      const item: Item = { id: id(), sideTabId: side, kind, value, source, originTabId, createdAt: Date.now() };
      itemList.push(item);
      contextChanged(side);
      return structuredClone(item);
    },
    async removeItem(itemId) {
      const at = itemList.findIndex((i) => i.id === itemId);
      if (at < 0) return;
      const [gone] = itemList.splice(at, 1);
      contextChanged(gone!.sideTabId);
    },
    async touchedFiles(side) {
      return structuredClone(touched.filter((f) => f.sideTabId === side).sort((a, b) => b.updatedAt - a.updatedAt));
    },
    async forgetFile(fileId) {
      const at = touched.findIndex((f) => f.id === fileId);
      if (at < 0) return;
      const [gone] = touched.splice(at, 1);
      contextChanged(gone!.sideTabId);
    },
    async searchPrompts(query, archived = false) {
      return library.search(query, archived);
    },
    async createPrompt(prompt) {
      return library.create(prompt);
    },
    async savePrompt(promptId, body) {
      return library.save(promptId, body);
    },
    async updatePrompt(promptId, name, tags) {
      return library.update(promptId, name, tags);
    },
    async promptVersions(promptId) {
      return library.versions(promptId);
    },
    async restorePrompt(promptId, version) {
      return library.restore(promptId, version);
    },
    async duplicatePrompt(promptId) {
      return library.duplicate(promptId);
    },
    async archivePrompt(promptId, archived) {
      return library.archive(promptId, archived);
    },
    async markPromptUsed(promptId) {
      library.markUsed(promptId);
    },
    async onPrompts(listener) {
      promptListeners.add(listener);
      return () => promptListeners.delete(listener);
    },
    async onContext(listener) {
      contextListeners.add(listener);
      return () => contextListeners.delete(listener);
    },
    async openUrl(url) {
      openedUrls.push(url);
    },
    files,
    dialogAnswers,
    dialogs,
    async newText(sideTabId, beside) {
      return showDoc(sideTabId, newDoc({ kind: "untitled" }, "Untitled", ""), beside);
    },
    async openFile(sideTabId, path, beside) {
      if (!path.startsWith("/")) throw new Error(`${path} is not an absolute path`);
      const open = [...docs.values()].find((d) => d.view.source.kind === "file" && d.view.source.path === path);
      if (open) return full(open);
      const text = files.get(path);
      return showDoc(sideTabId, newDoc({ kind: "file", path }, baseName(path), text ?? "", text === undefined), beside);
    },
    async openPath(sideTabId, _paneId, path, beside) {
      const abs = path.startsWith("/") ? path : path.startsWith("~/") ? `/Users/owner/${path.slice(2)}` : `${MOCK_CWD}/${path.replace(/^\.\//, "")}`;
      if (!files.has(abs)) throw new Error(`No file ${abs}`);
      return host.openFile(sideTabId, abs, beside);
    },
    async openPrompt(sideTabId, promptId, beside) {
      const open = [...docs.values()].find((d) => d.view.source.kind === "prompt" && d.view.source.promptId === promptId);
      if (open) return full(open);
      const p = [...library.search(""), ...library.search("", true)].find((x) => x.id === promptId);
      if (!p) throw new Error(`no prompt ${promptId}`);
      return showDoc(sideTabId, newDoc({ kind: "prompt", promptId }, p.name, p.body), beside);
    },
    async openVariable(sideTabId, name, beside) {
      const open = [...docs.values()].find((d) => d.view.source.kind === "variable" && d.view.source.sideTabId === sideTabId && d.view.source.name === name);
      if (open) return full(open);
      const v = (await host.variables(sideTabId)).find((x) => x.name === name);
      if (!v) throw new Error(`no variable ${name}`);
      if (v.mode === "secret") throw new Error(`${name} is a secret; its value never leaves the daemon`);
      const id = newDoc({ kind: "variable", sideTabId, name }, name, v.value ?? "");
      docs.get(id)!.view.sessionOnly = v.mode === "session";
      return showDoc(sideTabId, id, beside);
    },
    async sendText(pane, text) {
      if (!text.trim()) return;
      if (!ws.windows.some((w) => w.sideTabs.some((s) => s.tabs.some((t) => terminalsOf(t.layout).includes(pane))))) {
        throw new Error("that pane has no shell right now");
      }
      typeInto(pane, typeBytes(text, false));
    },
    async pasteImage(docId, bytes, ext) {
      const d = docOrFail(docId);
      const kind = { png: "png", jpg: "jpg", jpeg: "jpg", gif: "gif", webp: "webp" }[ext.toLowerCase()];
      if (!kind) throw new Error(`${ext} is not a png, jpg, gif or webp image`);
      const src = d.view.source;
      if (src.kind !== "file" && src.kind !== "untitled") throw new Error(`${d.view.title} has no folder to put a pasted image in`);
      const name = `pasted-${pastes++}.${kind}`;
      if (src.kind === "file") host.files.set(`${src.path.slice(0, src.path.lastIndexOf("/"))}/${name}`, `[${bytes.length} image bytes]`);
      return name;
    },
    async returnEdit(docId) {
      const d = docOrFail(docId);
      d.view.request = false;
      endWaits(docId);
    },
    async onFileDrop(listener) {
      dropListeners.add(listener);
      return () => dropListeners.delete(listener);
    },
    dropFiles(paths) {
      for (const l of dropListeners) l(paths);
    },
    touchFile(pane, path, change) {
      const side = ws.windows.flatMap((w) => w.sideTabs).find((s) => s.tabs.some((t) => panesOf(t.layout).includes(pane)));
      if (!side) return;
      const tab = side.tabs.find((t) => panesOf(t.layout).includes(pane))!;
      const shown = path.startsWith(`${MOCK_CWD}/`) ? path.slice(MOCK_CWD.length + 1) : path;
      const old = touched.find((f) => f.sideTabId === side.id && f.path === path);
      const updatedAt = Math.max(Date.now(), ...touched.map((f) => f.updatedAt + 1));
      if (old) Object.assign(old, { change: old.change === "added" ? "added" : change, originTabId: tab.id, updatedAt });
      else touched.push({ id: id(), sideTabId: side.id, path, shown, change, originTabId: tab.id, updatedAt });
      contextChanged(side.id);
    },
    async editWait(pane, path) {
      const side = ws.windows.flatMap((w) => w.sideTabs).find((s) => s.tabs.some((t) => panesOf(t.layout).includes(pane)));
      if (!side) throw new Error("this pane is not in the layout");
      const view = await host.openFile(side.id, path, pane);
      const d = docOrFail(view.id);
      d.view.request = true;
      for (const l of docListeners) l(meta(d));
      const shownIn = ws.windows
        .flatMap((w) => w.sideTabs.flatMap((s) => s.tabs))
        .flatMap((t) => panesOf(t.layout).filter((p) => leaf(t.layout, p)?.doc === view.id));
      for (const p of shownIn.slice(0, 1)) for (const l of revealListeners) l(p);
      return new Promise<void>((done) => waits.set(view.id, [...(waits.get(view.id) ?? []), done]));
    },
    async document(docId) {
      return full(docOrFail(docId));
    },
    async editDocument(docId, text, rev) {
      const d = docOrFail(docId);
      if (d.view.readOnly) throw new Error(`${d.view.title} opened read-only`);
      if (rev !== undefined && rev !== d.view.rev && text !== d.text) d.view.conflict = true;
      d.text = text;
      d.view.dirty = d.text !== d.saved;
      return meta(d);
    },
    async saveDocument(docId, path) {
      const d = docOrFail(docId);
      const src = d.view.source;
      if (path) {
        if (!path.startsWith("/")) throw new Error(`${path} is not an absolute path`);
        d.view.source = { kind: "file", path };
      } else if (src.kind === "untitled") {
        throw new Error(`${d.view.title} needs a file name to save`);
      } else if (src.kind === "file" && d.view.conflict) {
        throw new Error(`${d.view.title} changed on disk: pick Reload or Keep mine before saving`);
      } else if (src.kind === "prompt") {
        await library.save(src.promptId, d.text);
      } else if (src.kind === "variable") {
        await host.changeVariable(src.sideTabId, src.name, { value: d.text });
      }
      if (d.view.source.kind === "file") {
        files.set(d.view.source.path, d.text);
        d.view.title = baseName(d.view.source.path);
        d.view.missing = false;
        d.view.conflict = false;
      }
      d.saved = d.text;
      d.view.dirty = false;
      return meta(d);
    },
    async reloadDocument(docId) {
      const d = docOrFail(docId);
      if (d.view.source.kind !== "file") throw new Error(`${docId} is not a file`);
      d.text = d.saved = files.get(d.view.source.path) ?? "";
      d.view.dirty = d.view.conflict = false;
      d.view.rev++;
      return full(d);
    },
    async keepMine(docId) {
      const d = docOrFail(docId);
      if (d.view.source.kind !== "file") throw new Error(`${docId} is not a file`);
      d.saved = files.get(d.view.source.path) ?? "";
      d.view.conflict = false;
      d.view.dirty = d.text !== d.saved;
      return meta(d);
    },
    async onDocument(listener) {
      docListeners.add(listener);
      return () => docListeners.delete(listener);
    },
    writeFile(path, text) {
      files.set(path, text);
      for (const d of docs.values()) {
        if (d.view.source.kind !== "file" || d.view.source.path !== path || text === d.saved) continue;
        let out: DocView;
        if (d.view.dirty) {
          d.view.conflict = true;
          out = meta(d);
        } else {
          d.text = d.saved = text;
          d.view.rev++;
          out = full(d);
        }
        for (const l of docListeners) l(structuredClone(out));
      }
    },
    async listFiles() {
      const root = MOCK_CWD;
      const files_ = [...files.keys()].filter((p) => p.startsWith(`${root}/`)).map((p) => p.slice(root.length + 1));
      return { root, files: files_.sort(), truncated: false };
    },
    async chooseFile() {
      dialogs.push("open");
      return dialogAnswers.shift() ?? null;
    },
    async chooseSavePath() {
      dialogs.push("save");
      return dialogAnswers.shift() ?? null;
    },
    // Outside Tauri there is no app menu: chords come from key events, caught before xterm.js.
    async onAction(listener) {
      const target = globalThis as Partial<typeof globalThis>;
      if (!target.addEventListener || !target.removeEventListener) return () => {};
      const onKey = (e: KeyboardEvent) => {
        // A chord the app hands back to a field (see `passToField` in App.tsx) is untrusted.
        if (recording || !e.isTrusted) return;
        const action = actionFor(e, withBindings(ACTIONS, keyBindings));
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

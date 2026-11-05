// A Host without Tauri, for unit tests and Playwright runs of the Vite build (task 1.3). It
// plans commands like crates/core/src/state.rs, with readable ids (m1, m2, ...).

import type { Host, PaneView } from "./host";
import { actionFor } from "./keys";
import { hasSplit, isSessionId, leaf, panesOf, removePane, resizeSplit, splitPane } from "./layout";
import type { Command, CommandError, Patch, PatchBatch, SideTab, Snapshot, Tab, Workspace } from "./protocol";
import { applyBatch } from "./reducer";

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
      case "SplitPane": {
        const tab = tabOfPane(c.paneId);
        const layout = splitPane(tab.layout, c.paneId, c.dir, id(), id());
        if (layout === "small") return fail("PaneTooSmall", c.paneId);
        return [{ type: "LayoutChanged", tabId: tab.id, layout: layout! }];
      }
      case "ClosePane": {
        const tab = tabOfPane(c.paneId);
        const layout = removePane(tab.layout, c.paneId);
        if (layout) return [{ type: "LayoutChanged", tabId: tab.id, layout }];
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

  return {
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
}

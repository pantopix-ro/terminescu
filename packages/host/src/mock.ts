// A Host without Tauri, for unit tests and Playwright runs of the Vite build (task 1.3). It
// plans commands like crates/core/src/state.rs, with readable ids (m1, m2, ...).

import type { Host, PaneView } from "./host";
import type { Command, CommandError, Patch, PatchBatch, SideTab, Snapshot, Workspace } from "./protocol";
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
}

const encoder = new TextEncoder();

/**
 * A pretend shell per tab: a prompt, local echo and one canned answer per line. Enough for the
 * UI and Playwright to see a pane render, type and resize without a daemon.
 */
class MockShell {
  private line = "";
  private size = "80x24";
  constructor(private readonly tabId: string) {}
  prompt = () => `mock ${this.tabId} $ `;
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

  function firstStart(): Workspace {
    const windowId = id();
    return { windows: [{ id: windowId, sideTabs: [newSideTab()] }] };
  }
  function newSideTab(): SideTab {
    const tab = { id: id(), title: null };
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
        return [{ type: "TabCreated", sideTabId: c.sideTabId, index, tab: { id: id(), title: null } }];
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
    }
  }

  return {
    batches,
    openedUrls,
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
    async openPane(tabId, cols, rows, onOutput): Promise<PaneView> {
      holder(tabId);
      const shell = shells.get(tabId) ?? new MockShell(tabId);
      shells.set(tabId, shell);
      shell.resize(cols, rows);
      let open = true;
      const emit = (s: string) => {
        if (open && s) queueMicrotask(() => open && onOutput(encoder.encode(s)));
      };
      emit(shell.redraw());
      return {
        input: (data) => emit(shell.input(data)),
        resize: (c, r) => shell.resize(c, r),
        ack: () => {},
        close: () => {
          open = false;
        },
      };
    },
    async openUrl(url) {
      openedUrls.push(url);
    },
  };
}

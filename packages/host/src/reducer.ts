import type { Patch, PatchBatch, SideTab, Tab, Workspace } from "./protocol";

/** A patch that does not fit the workspace: the copy is stale and must resync. */
export class PatchError extends Error {}

function sideTabs(ws: Workspace): SideTab[] {
  return ws.windows.flatMap((w) => w.sideTabs);
}

function findSideTab(ws: Workspace, id: string): SideTab {
  const s = sideTabs(ws).find((s) => s.id === id);
  if (!s) throw new PatchError(`unknown side tab ${id}`);
  return s;
}

function findTab(ws: Workspace, id: string): Tab {
  const tab = sideTabs(ws).flatMap((s) => s.tabs).find((t) => t.id === id);
  if (!tab) throw new PatchError(`unknown tab ${id}`);
  return tab;
}

function takeTab(ws: Workspace, tabId: string): Tab {
  for (const s of sideTabs(ws)) {
    const i = s.tabs.findIndex((t) => t.id === tabId);
    if (i >= 0) return s.tabs.splice(i, 1)[0]!;
  }
  throw new PatchError(`unknown tab ${tabId}`);
}

function checkIndex(index: number, len: number) {
  if (!Number.isInteger(index) || index < 0 || index > len) {
    throw new PatchError(`index ${index} is out of range`);
  }
}

/** Applies one patch in place. Same rules as `apply` in crates/core/src/state.rs. */
export function applyPatch(ws: Workspace, p: Patch): void {
  switch (p.type) {
    case "WindowCreated": {
      if (ws.windows.some((w) => w.id === p.windowId)) throw new PatchError(`window ${p.windowId} exists`);
      ws.windows.push({ id: p.windowId, sideTabs: [] });
      return;
    }
    case "WindowClosed": {
      // Only an empty window closes: its tabs must have moved first.
      const i = ws.windows.findIndex((w) => w.id === p.windowId && w.sideTabs.length === 0);
      if (i < 0) throw new PatchError(`window ${p.windowId} is unknown or not empty`);
      ws.windows.splice(i, 1);
      return;
    }
    case "SideTabCreated": {
      const w = ws.windows.find((w) => w.id === p.windowId);
      if (!w) throw new PatchError(`unknown window ${p.windowId}`);
      checkIndex(p.index, w.sideTabs.length);
      w.sideTabs.splice(p.index, 0, structuredClone(p.sideTab));
      return;
    }
    case "SideTabClosed": {
      const w = ws.windows.find((w) => w.sideTabs.some((s) => s.id === p.sideTabId));
      if (!w) throw new PatchError(`unknown side tab ${p.sideTabId}`);
      w.sideTabs = w.sideTabs.filter((s) => s.id !== p.sideTabId);
      return;
    }
    case "TabCreated": {
      const s = findSideTab(ws, p.sideTabId);
      checkIndex(p.index, s.tabs.length);
      s.tabs.splice(p.index, 0, structuredClone(p.tab));
      return;
    }
    case "TabClosed":
      takeTab(ws, p.tabId);
      return;
    case "TabMoved": {
      const target = findSideTab(ws, p.toSideTabId);
      checkIndex(p.index, target.tabs.filter((t) => t.id !== p.tabId).length);
      const tab = takeTab(ws, p.tabId);
      target.tabs.splice(p.index, 0, tab);
      return;
    }
    case "TabRenamed":
      findTab(ws, p.tabId).title = p.title;
      return;
    case "LayoutChanged":
      findTab(ws, p.tabId).layout = structuredClone(p.layout);
      return;
    case "StatusSet": {
      const all = sideTabs(ws);
      const target = all.flatMap((s) => s.tabs).find((t) => t.id === p.id) ?? all.find((s) => s.id === p.id);
      if (!target) throw new PatchError(`unknown tab ${p.id}`);
      if (p.status) target.status = structuredClone(p.status);
      else delete target.status;
      return;
    }
  }
}

/** Returns a new workspace with the batch applied; the input stays untouched. */
export function applyBatch(ws: Workspace, batch: PatchBatch): Workspace {
  const next = structuredClone(ws);
  for (const p of batch.patches) applyPatch(next, p);
  return next;
}

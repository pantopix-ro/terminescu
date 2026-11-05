import {
  closestCenter,
  DndContext,
  DragOverlay,
  PointerSensor,
  pointerWithin,
  useSensor,
  useSensors,
  type CollisionDetection,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  ACTIONS,
  chordLabel,
  panesOf,
  terminalsOf,
  transcriptOf,
  type AgentUpdate,
  type ManualStatus,
  type Item,
  type Pin,
  type Shown,
  type StatusConfig,
  type Variable,
  type Prompt,
  type SavedCommand,
  type Command,
  type Host,
  type Workspace,
  type WorkspaceStore,
} from "@terminescu/host";
import {
  ContextPanel,
  EXTRACT_EVENT,
  INSERT_EVENT,
  insertText,
  ExtractDialog,
  PIN_EVENT,
  PromptLibrary,
  InsertDialog,
  SavePromptForm,
  paneEvent,
  REVEAL_ROWS_EVENT,
  quoteOf,
  TranscriptPane,
  type Agents,
  type Reveal,
  DROP_END,
  focusPane,
  Palette,
  SideTabList,
  sideTabName,
  SplitView,
  TabBar,
  tabName,
  TerminalPane,
  type ContextMode,
  type PaletteItem,
  SORTS,
  sortSideTabs,
  StatusEditor,
  StatusLine,
  StatusSettings,
  CommandsSection,
  playChime,
  type SortMode,
} from "@terminescu/ui";
import type { Terminal } from "@xterm/xterm";
import { useEffect, useRef, useState } from "react";
import { useStore } from "zustand";

export interface AppProps {
  windowId: string;
  host: Host;
  workspace: WorkspaceStore;
  /** Collects each pane's terminal, for Playwright on the mock host. */
  terminals?: Map<string, Terminal>;
}

/** Keeps a selection valid: the previous id while it still exists, else the first one. */
function pick(ids: string[], previous: string | undefined): string | undefined {
  return previous && ids.includes(previous) ? previous : ids[0];
}

const MODE_KEY = "terminescu.contextMode";
const FOLLOW_MS = 2000;
function loadMode(): ContextMode {
  try {
    return localStorage.getItem(MODE_KEY) === "pinned" ? "pinned" : "auto";
  } catch {
    return "auto";
  }
}
function saveMode(mode: ContextMode) {
  try {
    localStorage.setItem(MODE_KEY, mode);
  } catch {
    // Storage can be off; the mode then lasts until the window closes.
  }
}

const SORT_KEY = "terminescu.sideSort";
function loadSort(): SortMode {
  try {
    const v = localStorage.getItem(SORT_KEY);
    return (SORTS as readonly string[]).includes(v ?? "") ? (v as SortMode) : "manual";
  } catch {
    return "manual";
  }
}
function saveSort(mode: SortMode) {
  try {
    localStorage.setItem(SORT_KEY, mode);
  } catch {
    // Storage can be off; the sort then lasts until the window closes.
  }
}

type Dir = "left" | "right" | "up" | "down";

/** The pane next to `from` in a direction, by where the panes sit on screen. */
function neighbor(from: string, ids: string[], dir: Dir): string | undefined {
  const rect = (id: string) => document.querySelector(`.ao-tp[data-pane="${id}"]`)?.getBoundingClientRect();
  const a = rect(from);
  if (!a) return undefined;
  let best: { id: string; d: number } | undefined;
  for (const id of ids) {
    const b = id === from ? undefined : rect(id);
    if (!b) continue;
    const across = dir === "left" || dir === "right" ? b.top < a.bottom && b.bottom > a.top : b.left < a.right && b.right > a.left;
    const d = { left: a.left - b.right, right: b.left - a.right, up: a.top - b.bottom, down: b.top - a.bottom }[dir];
    if (!across || d < -1) continue;
    if (!best || d < best.d) best = { id, d };
  }
  return best?.id;
}

/** Sidebar targets under the pointer win; otherwise the nearest tab in the bar. */
const collide: CollisionDetection = (args) => {
  const under = pointerWithin(args);
  if (under.length) return under;
  const tabs = args.droppableContainers.filter((c) => !String(c.id).includes(":"));
  return closestCenter({ ...args, droppableContainers: tabs });
};

const selectable = new Set(["palette"]);
const listed = ACTIONS.filter((a) => !selectable.has(a.id) && !/^(tab|side)_\d$/.test(a.id));

export function App({ windowId, host, workspace, terminals }: AppProps) {
  const ws = useStore(workspace.store, (s) => s.workspace);
  const win = ws.windows.find((w) => w.id === windowId);
  const sideTabs = win?.sideTabs ?? [];

  // Shown statuses and the saved config, from the core (D15).
  const [statuses, setStatuses] = useState<Record<string, Shown>>({});
  const [statusConfig, setStatusConfig] = useState<StatusConfig>({ statuses: [], rules: [], builtins: {} });
  const [sort, setSort] = useState<SortMode>(loadSort);
  // The sidebar order; ⌃⌘1 to ⌃⌘9 follow what the person sees.
  const shownSides = sortSideTabs(sideTabs, statuses, sort, sideTabName);

  const [sideTabId, setSideTabId] = useState<string>();
  const activeSide = sideTabs.find((s) => s.id === pick(sideTabs.map((s) => s.id), sideTabId));
  const tabs = activeSide?.tabs ?? [];
  const [tabBySide, setTabBySide] = useState<Record<string, string>>({});
  const activeTab = tabs.find((t) => t.id === pick(tabs.map((t) => t.id), activeSide && tabBySide[activeSide.id]));
  const panes = activeTab ? panesOf(activeTab.layout) : [];
  const [focusByTab, setFocusByTab] = useState<Record<string, string>>({});
  const focused = pick(panes, activeTab && focusByTab[activeTab.id]);

  const [error, setError] = useState<string>();
  const [palette, setPalette] = useState(false);
  const [mode, setMode] = useState<ContextMode>(loadMode);
  const [dragging, setDragging] = useState<string>();
  const [sheet, setSheet] = useState<"status" | "settings" | "prompts" | "insert" | "save-prompt">();
  const [toSave, setToSave] = useState("");
  const [libraryKey, setLibraryKey] = useState<string>();

  const send = (c: Command): Promise<boolean> =>
    host.send(c).then(
      () => true,
      (e: unknown) => {
        // A refused command creates nothing to select.
        follow.current = undefined;
        setError(JSON.stringify(e));
        return false;
      },
    );
  const focusedRef = useRef(focused);
  focusedRef.current = focused;
  // Runs after React re-rendered, so it gives the keyboard to the pane focused by then.
  const refocus = () => setTimeout(() => focusedRef.current && focusPane(focusedRef.current));
  const selectTab = (sideId: string, tabId: string) => {
    setSideTabId(sideId);
    setTabBySide((m) => ({ ...m, [sideId]: tabId }));
  };
  const focusIn = (tabId: string, paneId: string) => setFocusByTab((m) => ({ ...m, [tabId]: paneId }));

  // What a command created gets selected once its patch arrives: a check that runs on every
  // workspace change until it finds its target.
  const follow = useRef<((w: Workspace) => boolean) | undefined>(undefined);
  // A check gives up after FOLLOW_MS, so a late unrelated change never jumps the selection.
  const setFollow = (check: (w: Workspace) => boolean) => {
    const until = Date.now() + FOLLOW_MS;
    follow.current = (w) => Date.now() > until || check(w);
  };
  const sidesOf = (w: Workspace) => w.windows.find((x) => x.id === windowId)?.sideTabs ?? [];
  const followNewSide = () => {
    const before = new Set(sideTabs.map((s) => s.id));
    setFollow((w) => {
      const added = sidesOf(w).find((s) => !before.has(s.id));
      if (added) setSideTabId(added.id);
      return !!added;
    });
  };
  const followTab = (tabId: string) => {
    setFollow((w) => {
      const side = sidesOf(w).find((s) => s.tabs.some((t) => t.id === tabId));
      if (side) selectTab(side.id, tabId);
      return !!side;
    });
  };
  const followNewTab = (sideId: string) => {
    const before = new Set(tabs.map((t) => t.id));
    setFollow((w) => {
      const added = sidesOf(w)
        .find((s) => s.id === sideId)
        ?.tabs.find((t) => !before.has(t.id));
      if (added) selectTab(sideId, added.id);
      return !!added;
    });
  };
  const followNewPane = (tabId: string) => {
    const before = new Set(panes);
    setFollow((w) => {
      const tab = sidesOf(w)
        .flatMap((s) => s.tabs)
        .find((t) => t.id === tabId);
      const added = tab && panesOf(tab.layout).find((p) => !before.has(p));
      if (added) focusIn(tabId, added);
      return !!added;
    });
  };

  // A closed pane hands focus to the pane before it, as the core hands it its space.
  const lastPanes = useRef<{ tab?: string; panes: string[] }>({ panes: [] });
  useEffect(() => {
    if (follow.current?.(ws)) follow.current = undefined;
    const last = lastPanes.current;
    const gone = activeTab && focusByTab[activeTab.id];
    if (activeTab && last.tab === activeTab.id && gone && !panes.includes(gone)) {
      const at = last.panes.indexOf(gone);
      const next = last.panes.slice(0, Math.max(at, 0)).reverse().find((p) => panes.includes(p)) ?? panes[0];
      if (next) focusIn(activeTab.id, next);
    }
    lastPanes.current = { tab: activeTab?.id, panes };
  }, [ws]);

  const act = (id: string) => {
    const nth = /^(tab|side)_(\d)$/.exec(id);
    if (nth) {
      const list = nth[1] === "tab" ? tabs : shownSides;
      const n = Number(nth[2]);
      const target = n === 9 ? list.at(-1) : list[n - 1];
      if (!target) return;
      if (nth[1] === "side") setSideTabId(target.id);
      else if (activeSide) selectTab(activeSide.id, target.id);
      return;
    }
    switch (id) {
      case "new_tab":
        if (!activeSide) return;
        followNewTab(activeSide.id);
        return void send({ type: "CreateTab", sideTabId: activeSide.id });
      case "new_side_tab":
        followNewSide();
        return void send({ type: "CreateSideTab", windowId });
      case "split_right":
      case "split_down":
        if (!activeTab || !focused) return;
        followNewPane(activeTab.id);
        return void send({ type: "SplitPane", paneId: focused, dir: id === "split_right" ? "row" : "column" });
      case "close":
        if (focused) void send({ type: "ClosePane", paneId: focused });
        return;
      case "open_transcript": {
        const session = focused && agents[focused]?.session;
        if (!activeTab || !focused) return;
        if (!session) return setError("No Claude session runs in this pane.");
        followNewPane(activeTab.id);
        return void send({ type: "OpenTranscript", paneId: focused, session });
      }
      case "pin":
      case "extract":
        // The focused pane, transcript or terminal, acts on its selection.
        if (focused) paneEvent(focused, new Event(id === "pin" ? PIN_EVENT : EXTRACT_EVENT));
        return;
      case "palette":
        return setPalette((p) => !p);
      case "status_editor":
        if (activeSide) setSheet((s) => (s === "status" ? undefined : "status"));
        return;
      case "settings":
        return setSheet((s) => (s === "settings" ? undefined : "settings"));
      case "insert":
        if (!focused || !activeTab || transcriptOf(activeTab.layout, focused)) return setError("Focus a terminal pane to insert.");
        return setSheet((s) => (s === "insert" ? undefined : "insert"));
      case "prompt_library":
        setLibraryKey(undefined);
        return setSheet((s) => (s === "prompts" ? undefined : "prompts"));
      case "context_panel":
        return changeMode(mode === "pinned" ? "auto" : "pinned");
      case "jump_waiting":
        return void host.jumpWaiting().then(
          (pane) => pane ?? setNotice("No session is waiting."),
          (e: unknown) => setError(String(e)),
        );
      case "focus_left":
      case "focus_right":
      case "focus_up":
      case "focus_down": {
        const next = focused && neighbor(focused, panes, id.slice(6) as Dir);
        if (next && activeTab) focusIn(activeTab.id, next);
        return;
      }
    }
  };
  // Claude sessions by pane, from hook events in the core (D34).
  const [agents, setAgents] = useState<Agents>({});
  useEffect(() => {
    let off: (() => void) | undefined;
    let gone = false;
    const apply = (u: AgentUpdate) =>
      setAgents((m) => {
        const next = { ...m };
        if (u.agent) next[u.pane] = u.agent;
        else delete next[u.pane];
        return next;
      });
    void host.onAgent(apply).then((o) => (gone ? o() : (off = o)));
    void host.agents().then((all) => !gone && all.forEach(apply));
    return () => {
      gone = true;
      off?.();
    };
  }, [host]);

  useEffect(() => {
    let gone = false;
    const offs: (() => void)[] = [];
    const keep = (o: () => void) => (gone ? o() : offs.push(o));
    const loadConfig = () =>
      void host
        .statusConfig()
        .then((c) => !gone && setStatusConfig(c))
        .catch((e: unknown) => !gone && setError(String(e)));
    // An event is newer than the first read, so a late read never overwrites one.
    let heard = false;
    void host
      .onStatuses((m) => {
        heard = true;
        if (!gone) setStatuses(m);
      })
      .then(keep);
    void host.onStatusConfig(loadConfig).then(keep);
    void host.statuses().then((m) => !gone && !heard && setStatuses(m));
    loadConfig();
    return () => {
      gone = true;
      offs.forEach((o) => o());
    };
  }, [host]);
  const setStatus = (targetId: string, status: ManualStatus | null) => send({ type: "SetStatus", targetId, status });
  const closeSheet = () => {
    setSheet(undefined);
    refocus();
  };
  const report = (e: unknown) => {
    setError(String(e));
    throw e;
  };

  // The active side tab's variables, links, text and pins, read again whenever the core says
  // they changed.
  type Ctx = { side?: string; variables: Variable[]; items: Item[]; pins: Pin[] };
  const [ctx, setCtx] = useState<Ctx>({ variables: [], items: [], pins: [] });
  const sideId = activeSide?.id;
  useEffect(() => {
    if (!sideId) return;
    let gone = false;
    let off: (() => void) | undefined;
    const load = () =>
      void Promise.all([host.variables(sideId), host.items(sideId), host.pins(sideId)])
        .then(([variables, items, pins]) => !gone && setCtx({ side: sideId, variables, items, pins }))
        .catch((e: unknown) => !gone && setError(String(e)));
    load();
    void host.onContext((s) => s === sideId && load()).then((o) => (gone ? o() : (off = o)));
    return () => {
      gone = true;
      off?.();
    };
  }, [host, sideId]);
  const shownCtx: Ctx = ctx.side === sideId ? ctx : { variables: [], items: [], pins: [] };

  // The library, last used first, for the panel's Prompts section and `⌘I` (D14).
  const [prompts, setPrompts] = useState<Prompt[]>([]);
  useEffect(() => {
    let gone = false;
    let off: (() => void) | undefined;
    const load = () =>
      void host
        .searchPrompts("")
        .then((p) => !gone && setPrompts(p))
        .catch((e: unknown) => !gone && setError(String(e)));
    load();
    void host.onPrompts(load).then((o) => (gone ? o() : (off = o)));
    return () => {
      gone = true;
      off?.();
    };
  }, [host]);

  // Saved commands of the side tab in front, then the global ones (D24).
  const [commands, setCommands] = useState<SavedCommand[]>([]);
  const commandSide = activeSide?.id ?? null;
  useEffect(() => {
    let gone = false;
    let off: (() => void) | undefined;
    const load = () =>
      void host
        .commands(commandSide)
        .then((c) => !gone && setCommands(c))
        .catch((e: unknown) => !gone && setError(String(e)));
    load();
    void host.onCommands(load).then((o) => (gone ? o() : (off = o)));
    return () => {
      gone = true;
      off?.();
    };
  }, [host, commandSide]);
  const runCommand = async (c: SavedCommand, run: boolean, extra: Record<string, string>) => {
    if (!activeSide) throw new Error("No side tab.");
    const paneId = focused && activeTab && !transcriptOf(activeTab.layout, focused) ? focused : null;
    const out = await host.runSavedCommand({ windowId, sideTabId: activeSide.id, id: c.id, paneId, run, extra });
    if (out.kind === "done" && out.pane) revealPane(out.pane);
    return out;
  };

  // A pin to show: the transcript pane of its session scrolls to it; a terminal pin scrolls its
  // pane, when that pane is still open in this side tab.
  const [reveal, setReveal] = useState<Reveal & { session: string }>();
  const openPin = (pin: Pin) => {
    if (pin.paneId) {
      const tab = activeSide?.tabs.find((t) => terminalsOf(t.layout).includes(pin.paneId!));
      if (!tab) return setError("That pane is closed; the pin keeps its text.");
      focusIn(tab.id, pin.paneId);
      const rows = [pin.rowStart ?? 0, pin.rowEnd ?? pin.rowStart ?? 0];
      setTimeout(() => paneEvent(pin.paneId!, new CustomEvent(REVEAL_ROWS_EVENT, { detail: rows })));
      return;
    }
    const session = pin.session;
    if (!session || !pin.entryId) return;
    const shown = activeTab && panesOf(activeTab.layout).some((p) => transcriptOf(activeTab.layout, p) === session);
    if (!shown && activeTab && focused) {
      followNewPane(activeTab.id);
      void send({ type: "OpenTranscript", paneId: focused, session });
    }
    setReveal({ ...pin, entryId: pin.entryId, session, nonce: Date.now() });
  };

  // `⌘E` opens one dialog, whichever pane the text came from (D13).
  const [extracting, setExtracting] = useState<{ side: string; tab: string; value: string; from: string }>();
  const startExtract = (tabId: string, value: string, from: string) => {
    const side = sideOfTab(tabId);
    if (side) setExtracting({ side, tab: tabId, value, from });
  };
  const pinTerminal = (tabId: string, paneId: string, text: string, rows: [number, number]) => {
    const side = sideOfTab(tabId);
    if (!side) return;
    void host
      .addPin({
        sideTabId: side,
        originTabId: tabId,
        session: null,
        entryId: null,
        paneId,
        rowStart: rows[0],
        rowEnd: rows[1],
        exact: text,
        prefix: null,
        suffix: null,
        start: null,
        end: null,
      })
      .catch((e: unknown) => setError(String(e)));
  };
  // Inserts go to the focused terminal only, never with Enter unless `send` (D13).
  const insertInto = (text: string, send = false) => {
    if (!focused || !activeTab || transcriptOf(activeTab.layout, focused)) return false;
    paneEvent(focused, new CustomEvent(INSERT_EVENT, { detail: { text, send } }));
    return true;
  };
  // A secret goes in as `$NAME` (D21, D35). In a Claude pane the person learns where the value
  // goes and where it must not: a raw value typed into a prompt stays in Claude's transcript.
  const secretNote = (names: string[]) => {
    if (focused && agents[focused] && names.length)
      setNotice(
        `Claude sees ${names.map((n) => `$${n}`).join(", ")}; its commands read the value from the environment. Typed raw, a value stays in Claude's transcript.`,
      );
  };
  const insertVariable = (v: Variable) => {
    if (insertInto(insertText(v)) && v.mode === "secret") secretNote([v.name]);
  };
  const insertChosen = (text: string, send: boolean, prompt?: Prompt) => {
    closeSheet();
    if (!insertInto(text, send)) return setError("Focus a terminal pane to insert.");
    if (prompt) void host.markPromptUsed(prompt.id).catch((e: unknown) => setError(String(e)));
    secretNote(shownCtx.variables.filter((v) => v.mode === "secret" && text.includes(`$${v.name}`)).map((v) => v.name));
  };
  const [notice, setNotice] = useState<string>();
  const sideOfTab = (tabId: string) => sideTabs.find((s) => s.tabs.some((t) => t.id === tabId))?.id;

  // Shows a pane: selects its side tab and tab and focuses it, now or once its patch arrives.
  // `⌘⇧A` lands here in the window holding the pane, and so does a command run in a new tab.
  const revealPane = (paneId: string) => {
    const show = (w: Workspace) => {
      const side = sidesOf(w).find((s) => s.tabs.some((t) => panesOf(t.layout).includes(paneId)));
      const tab = side?.tabs.find((t) => panesOf(t.layout).includes(paneId));
      if (!side || !tab) return false;
      selectTab(side.id, tab.id);
      focusIn(tab.id, paneId);
      setTimeout(() => focusPane(paneId));
      return true;
    };
    if (!show(workspace.store.getState().workspace)) setFollow(show);
  };
  const revealRef = useRef(revealPane);
  revealRef.current = revealPane;
  useEffect(() => {
    const offs: (() => void)[] = [];
    let gone = false;
    const keep = (o: () => void) => (gone ? o() : offs.push(o));
    void host.onRevealPane((p) => revealRef.current(p)).then(keep);
    void host.onChime(playChime).then(keep);
    return () => {
      gone = true;
      offs.forEach((o) => o());
    };
  }, [host]);

  const actRef = useRef(act);
  actRef.current = act;
  useEffect(() => {
    let off: (() => void) | undefined;
    let gone = false;
    void host.onAction((a) => actRef.current(a)).then((o) => (gone ? o() : (off = o)));
    return () => {
      gone = true;
      off?.();
    };
  }, [host]);

  const changeMode = (m: ContextMode) => {
    setMode(m);
    saveMode(m);
  };

  const moveTab = (tabId: string, target: { side: string } | { newAt: number }) => {
    if ("side" in target) {
      const to = sideTabs.find((s) => s.id === target.side);
      if (!to) return;
      followTab(tabId);
      return void send({ type: "MoveTab", tabId, toSideTabId: to.id, index: to.tabs.length });
    }
    followTab(tabId);
    void send({ type: "MoveTabToNewSideTab", tabId, windowId, index: target.newAt });
  };

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }));
  const onDragEnd = ({ active, over }: DragEndEvent) => {
    setDragging(undefined);
    refocus();
    const tabId = String(active.id);
    const at = over && String(over.id);
    if (!at || !activeSide) return;
    // The only tab of a side tab dropped next to it would make the same sidebar again.
    const idx = sideTabs.findIndex((s) => s.id === activeSide.id);
    const gap = at === DROP_END ? sideTabs.length : at.startsWith("gap:") ? Number(at.slice(4)) : -1;
    if (activeSide.tabs.length === 1 && (gap === idx || gap === idx + 1)) return;
    if (at.startsWith("into:")) {
      const side = at.slice(5);
      if (side !== activeSide.id) moveTab(tabId, { side });
    } else if (at === DROP_END) {
      moveTab(tabId, { newAt: sideTabs.length });
    } else if (at.startsWith("gap:")) {
      moveTab(tabId, { newAt: Number(at.slice(4)) });
    } else {
      const from = tabs.findIndex((t) => t.id === tabId);
      const to = tabs.findIndex((t) => t.id === at);
      if (from >= 0 && to >= 0 && from !== to) void send({ type: "MoveTab", tabId, toSideTabId: activeSide.id, index: to });
    }
  };

  const paletteItems: PaletteItem[] = [
    ...listed.map((a) => ({ id: a.id, label: a.label, group: "Actions", hint: chordLabel(a.accel), run: () => act(a.id) })),
    ...(activeTab
      ? [
          { id: "move:new", label: "New side tab", group: "Move tab to…", run: () => moveTab(activeTab.id, { newAt: sideTabs.length }) },
          ...sideTabs
            .filter((s) => s.id !== activeSide?.id)
            .map((s) => ({ id: `move:${s.id}`, label: sideTabName(s), group: "Move tab to…", run: () => moveTab(activeTab.id, { side: s.id }) })),
        ]
      : []),
    ...sideTabs.map((s) => ({ id: `go:${s.id}`, label: sideTabName(s), group: "Go to side tab", run: () => setSideTabId(s.id) })),
  ];

  const dragTab = dragging ? tabs.find((t) => t.id === dragging) : undefined;
  const multi = panes.length > 1;

  return (
    <>
      <DndContext
        sensors={sensors}
        collisionDetection={collide}
        onDragStart={({ active }) => {
          setDragging(String(active.id));
          // Esc cancels the drag; the keyboard must not be in a terminal for that.
          (document.activeElement as HTMLElement | null)?.blur();
        }}
        onDragEnd={onDragEnd}
        onDragCancel={() => {
          setDragging(undefined);
          refocus();
        }}
      >
        <div className="app" data-context={mode}>
          <aside className="app-side">
            <div className="app-traffic" data-tauri-drag-region />
            <SideTabList
              sideTabs={shownSides}
              statuses={statuses}
              builtins={statusConfig.builtins}
              sort={sort}
              onSort={(m) => {
                setSort(m);
                saveSort(m);
              }}
              activeId={activeSide?.id}
              activeTabId={activeTab?.id}
              dragging={!!dragging}
              agents={agents}
              onSelect={(id) => {
                setSideTabId(id);
                refocus();
              }}
              onSelectTab={(side, tab) => {
                selectTab(side, tab);
                refocus();
              }}
              onNew={() => act("new_side_tab")}
            />
            <CommandsSection
              commands={commands}
              enabled={!!activeSide}
              onAdd={(d) =>
                host.createCommand({
                  sideTabId: d.global ? null : (activeSide?.id ?? null),
                  name: d.name,
                  body: d.body,
                  target: d.target,
                  launcher: false,
                })
              }
              onDelete={(id) => host.deleteCommand(id)}
              onRun={runCommand}
            />
          </aside>
          <main className="app-main">
            <TabBar
              tabs={tabs}
              activeId={activeTab?.id}
              agents={agents}
              statuses={statuses}
              builtins={statusConfig.builtins}
              onSelect={(id) => {
                if (activeSide) selectTab(activeSide.id, id);
                refocus();
              }}
              onClose={(tabId) => void send({ type: "CloseTab", tabId })}
              onNew={() => act("new_tab")}
            />
            <section className="app-pane">
              {activeTab && (
                <SplitView
                  key={activeTab.id}
                  layout={activeTab.layout}
                  onResize={(splitId, sizes) => send({ type: "ResizeSplit", splitId, sizes })}
                  renderPane={(paneId, resume, slot, transcript) => (
                    <section
                      className={`ao-tp${multi && paneId === focused ? " focused" : ""}`}
                      data-pane={paneId}
                      onPointerDown={() => focusIn(activeTab.id, paneId)}
                    >
                      {multi && (
                        <div className="ao-tp-head">
                          <i className={`ao-i x1 ${transcript ? "ao-i-agent" : "ao-i-term"}`} />
                          <span>{transcript ? "transcript" : "shell"}</span>
                          <span className="grow" />
                          <button type="button" className="tp-close" aria-label="Close pane" onClick={() => void send({ type: "ClosePane", paneId })}>
                            ×
                          </button>
                        </div>
                      )}
                      {resume && (
                        <div className="ao-bar-wait resume-bar" role="status">
                          <span className="w">Claude session {resume}</span>
                          <span className="grow small ao-dim">ended with the last restart</span>
                          <button type="button" className="ao-btn primary" onClick={() => void send({ type: "ResumePane", paneId })}>
                            Resume
                          </button>
                          <button type="button" className="ao-btn ghost" onClick={() => void send({ type: "DismissResume", paneId })}>
                            Dismiss
                          </button>
                        </div>
                      )}
                      <div className="ao-tp-body">
                        {transcript ? (
                          <TranscriptPane
                            paneId={paneId}
                            session={transcript}
                            read={(offset) => host.readTranscript(transcript, offset)}
                            pin={(entry, range) => {
                              const side = sideOfTab(activeTab.id);
                              if (!side) return Promise.resolve();
                              const q = range ? quoteOf(entry.text, ...range) : undefined;
                              return host.addPin({
                                sideTabId: side,
                                originTabId: activeTab.id,
                                session: transcript,
                                entryId: entry.id,
                                paneId: null,
                                rowStart: null,
                                rowEnd: null,
                                exact: q ? q.exact : entry.text,
                                prefix: q?.prefix ?? null,
                                suffix: q?.suffix ?? null,
                                start: range?.[0] ?? null,
                                end: range?.[1] ?? null,
                              });
                            }}
                            extract={(value) => startExtract(activeTab.id, value, "transcript")}
                            savePrompt={(text) => {
                              setToSave(text);
                              setSheet("save-prompt");
                            }}
                            reveal={reveal?.session === transcript ? reveal : undefined}
                            onFocus={() => focusIn(activeTab.id, paneId)}
                          />
                        ) : (
                          slot
                        )}
                      </div>
                    </section>
                  )}
                />
              )}
              {!activeTab && <p className="small app-empty">No side tabs. Press + in the sidebar.</p>}
              {notice && (
                <p className="small app-note" role="status" onClick={() => setNotice(undefined)}>
                  {notice}
                </p>
              )}
              {error && (
                <p className="small app-error" role="alert" onClick={() => setError(undefined)}>
                  {error}
                </p>
              )}
            </section>
          </main>
          <ContextPanel
            mode={mode}
            onMode={changeMode}
            owner={activeSide && sideTabName(activeSide)}
            variables={shownCtx.variables}
            items={shownCtx.items}
            pins={shownCtx.pins}
            onOpenPin={openPin}
            onRemovePin={(p) => void host.removePin(p.id).catch((e: unknown) => setError(String(e)))}
            onAddVariable={(v) => (sideId ? host.addVariable(sideId, { ...v, originTabId: activeTab?.id ?? null }) : Promise.resolve())}
            onChangeVariable={(name, change) => (sideId ? host.changeVariable(sideId, name, change) : Promise.resolve())}
            onInsertVariable={focused && activeTab && !transcriptOf(activeTab.layout, focused) ? insertVariable : undefined}
            onRemoveVariable={(name) => (sideId ? host.removeVariable(sideId, name) : Promise.resolve())}
            onAddItem={(kind, value) => (sideId ? host.addItem(sideId, kind, value, "pasted", activeTab?.id ?? null) : Promise.resolve())}
            onRemoveItem={(id) => host.removeItem(id)}
            prompts={prompts}
            onOpenPrompt={(key) => {
              setLibraryKey(key);
              setSheet("prompts");
            }}
            onOpenUrl={(url) => void host.openUrl(url).catch((e: unknown) => setError(String(e)))}
          />
          <StatusLine
            agents={agents}
            sideTabs={sideTabs.length}
            tabs={sideTabs.reduce((n, s) => n + s.tabs.length, 0)}
            onJump={() => act("jump_waiting")}
          />
        </div>
        <DragOverlay dropAnimation={null}>
          {dragTab && (
            <div className="ao-tab ghost">
              <span className="n">{tabName(dragTab)}</span>
            </div>
          )}
        </DragOverlay>
      </DndContext>
      {/* Every pane of the window keeps its terminal; the active tab's panes show (D33). */}
      {sideTabs.flatMap((side) =>
        side.tabs.flatMap((tab) =>
          terminalsOf(tab.layout).map((paneId) => (
            <TerminalPane
              key={paneId}
              paneId={paneId}
              visible={tab.id === activeTab?.id}
              focused={paneId === focused}
              onFocus={() => focusIn(tab.id, paneId)}
              open={(cols, rows, onOutput) => host.openPane(paneId, cols, rows, onOutput)}
              openUrl={(url) => void host.openUrl(url).catch((e: unknown) => setError(String(e)))}
              onTerminal={terminals && ((t) => (t ? terminals.set(paneId, t) : terminals.delete(paneId)))}
              onSelection={(how, text, rows) =>
                how === "pin" ? pinTerminal(tab.id, paneId, text, rows) : startExtract(tab.id, text, "terminal")
              }
            />
          )),
        ),
      )}
      {extracting && (
        <ExtractDialog
          value={extracting.value}
          from={extracting.from}
          taken={ctx.side === extracting.side ? ctx.variables.map((v) => v.name) : []}
          save={(v) =>
            host.addVariable(extracting.side, { ...v, value: extracting.value, source: "extracted", originTabId: extracting.tab })
          }
          onClose={() => {
            setExtracting(undefined);
            refocus();
          }}
        />
      )}
      {sheet === "status" && activeSide && (
        <StatusEditor
          side={activeSide}
          tab={activeTab}
          sideName={sideTabName(activeSide)}
          tabName={activeTab && tabName(activeTab)}
          statuses={statuses}
          config={statusConfig}
          onSet={setStatus}
          onSave={(side, look) => host.saveStatus(side, look).catch(report)}
          onMakeGlobal={(id) => host.makeGlobal(id).catch(report)}
          onSettings={() => setSheet("settings")}
          onClose={closeSheet}
        />
      )}
      {sheet === "insert" && (
        <InsertDialog
          prompts={prompts}
          variables={shownCtx.variables}
          pins={shownCtx.pins}
          onInsert={insertChosen}
          onSaveVariable={(name, value) =>
            sideId
              ? host.addVariable(sideId, { name, value, mode: "saved", ttlSecs: null, source: "typed", originTabId: activeTab?.id ?? null })
              : Promise.resolve()
          }
          onSavePrompt={(p) => host.createPrompt(p).catch(report)}
          onError={setError}
          onClose={closeSheet}
        />
      )}
      {sheet === "save-prompt" && (
        <div className="sheet-back" onPointerDown={closeSheet}>
          <div
            className="ao-frame insert-dialog"
            role="dialog"
            aria-label="Save as prompt"
            onPointerDown={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.key === "Escape" && closeSheet()}
          >
            <div className="ao-frame-body">
              <SavePromptForm
                text={toSave}
                variables={shownCtx.variables}
                onSave={(p) =>
                  host.createPrompt(p).then((saved) => {
                    setLibraryKey(saved.key);
                    setSheet("prompts");
                  })
                }
                onCancel={closeSheet}
              />
            </div>
          </div>
        </div>
      )}
      {sheet === "prompts" && <PromptLibrary host={host} selectKey={libraryKey} onError={setError} onClose={closeSheet} />}
      {sheet === "settings" && <StatusSettings config={statusConfig} host={host} onError={setError} onClose={closeSheet} />}
      {palette && (
        <Palette
          items={paletteItems}
          onClose={() => {
            setPalette(false);
            refocus();
          }}
        />
      )}
    </>
  );
}

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
  type Pin,
  type Shown,
  type StatusConfig,
  type Variable,
  type Command,
  type Host,
  type Workspace,
  type WorkspaceStore,
} from "@terminescu/host";
import {
  ContextPanel,
  EXTRACT_EVENT,
  PIN_EVENT,
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
  StatusSettings,
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
  const [sheet, setSheet] = useState<"status" | "settings">();

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
        // The focused transcript pane acts on its selection.
        document
          .querySelector(`[data-transcript="${CSS.escape(focused ?? "")}"]`)
          ?.dispatchEvent(new Event(id === "pin" ? PIN_EVENT : EXTRACT_EVENT));
        return;
      case "palette":
        return setPalette((p) => !p);
      case "status_editor":
        if (activeSide) setSheet((s) => (s === "status" ? undefined : "status"));
        return;
      case "settings":
        return setSheet((s) => (s === "settings" ? undefined : "settings"));
      case "context_panel":
        return changeMode(mode === "pinned" ? "auto" : "pinned");
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

  // The active side tab's variables and pins, read again whenever the core says they changed.
  const [ctx, setCtx] = useState<{ side?: string; variables: Variable[]; pins: Pin[] }>({ variables: [], pins: [] });
  const sideId = activeSide?.id;
  useEffect(() => {
    if (!sideId) return;
    let gone = false;
    let off: (() => void) | undefined;
    const load = () =>
      void Promise.all([host.variables(sideId), host.pins(sideId)])
        .then(([variables, pins]) => !gone && setCtx({ side: sideId, variables, pins }))
        .catch((e: unknown) => !gone && setError(String(e)));
    load();
    void host.onContext((s) => s === sideId && load()).then((o) => (gone ? o() : (off = o)));
    return () => {
      gone = true;
      off?.();
    };
  }, [host, sideId]);
  const shownCtx = ctx.side === sideId ? ctx : { variables: [], pins: [] };

  // A pin to show: the transcript pane of its session scrolls to it.
  const [reveal, setReveal] = useState<Reveal & { session: string }>();
  const openPin = (pin: Pin) => {
    const shown = activeTab && panesOf(activeTab.layout).some((p) => transcriptOf(activeTab.layout, p) === pin.session);
    if (!shown && activeTab && focused) {
      followNewPane(activeTab.id);
      void send({ type: "OpenTranscript", paneId: focused, session: pin.session });
    }
    setReveal({ ...pin, session: pin.session, nonce: Date.now() });
  };
  const sideOfTab = (tabId: string) => sideTabs.find((s) => s.tabs.some((t) => t.id === tabId))?.id;

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
                                session: transcript,
                                entryId: entry.id,
                                exact: q ? q.exact : entry.text,
                                prefix: q?.prefix ?? null,
                                suffix: q?.suffix ?? null,
                                start: range?.[0] ?? null,
                                end: range?.[1] ?? null,
                              });
                            }}
                            extract={(name, value) => {
                              const side = sideOfTab(activeTab.id);
                              return side ? host.extractVariable(side, name, value) : Promise.resolve();
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
            pins={shownCtx.pins}
            onOpenPin={openPin}
            onRemovePin={(p) => void host.removePin(p.id).catch((e: unknown) => setError(String(e)))}
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
            />
          )),
        ),
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

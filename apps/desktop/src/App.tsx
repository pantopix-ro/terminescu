import {
  closestCenter,
  DndContext,
  DragOverlay,
  PointerSensor,
  pointerWithin,
  useDraggable,
  useSensor,
  useSensors,
  type CollisionDetection,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  ACTIONS,
  chordEvent,
  chordLabel,
  DEFAULT_SETTINGS,
  SIDEBAR_MAX,
  SIDEBAR_MIN,
  SIDEBAR_WIDTH,
  stepZoom,
  withBindings,
  docOf,
  hasSplit,
  leaf,
  panesOf,
  worksBesides,
  resizeSplit,
  terminalsOf,
  transcriptOf,
  type AgentUpdate,
  type DocView,
  type FileList,
  type Tab,
  type AppSettings,
  type ActionDef,
  type KeyBindings,
  type ManualStatus,
  type Item,
  type TouchedFile,
  type Pin,
  type Shown,
  type StatusConfig,
  type Variable,
  type Prompt,
  type SavedCommand,
  type Command,
  type DragItem,
  type DragPoint,
  type DragEnd,
  type DockSide,
  dockPlace,
  splitAt,
  type EditAction,
  type Host,
  type Workspace,
  type WorkspaceStore,
  type MergeClash,
  type SideTab,
  type MergeResolution,
  MERGE_UNDO_MS,
} from "@terminescu/host";
import {
  Tip,
  useTip,
  KidIcon,
  tabAgent,
  composeView,
  fitView,
  folderLines,
  hideRow,
  replaceRow,
  rowsOf,
  showBeside,
  viewSplitId,
  type View,
  ContextPanel,
  EXTRACT_EVENT,
  INSERT_EVENT,
  insertText,
  ExtractDialog,
  MergeDialog,
  PasteDialog,
  shellPaths,
  ContextMenu,
  type MenuEntry,
  chipOf,
  type MenuTarget,
  PIN_EVENT,
  PromptLibrary,
  InsertDialog,
  SavePromptForm,
  paneEvent,
  REVEAL_ROWS_EVENT,
  type Agents,
  type Labels,
  type Box,
  dockBox,
  dockZone,
  paneSubtitle,
  tabActivities,
  tabSubtitles,
  withLiveNames,
  DROP_END,
  tabOfDrag,
  focusPane,
  Palette,
  FileFinder,
  FindAllSheet,
  type FindHit,
  SideTabList,
  sideTabName,
  SearchBar,
  type SearchBarHandle,
  SplitView,
  TabBar,
  tabName,
  TerminalPane,
  trapModalFocus,
  TextPane,
  terminalSelection,
  clearTerminal,
  selectAllIn,
  promptAction,
  textPane,
  type MountEditor,
  type ContextMode,
  type PaletteItem,
  SORTS,
  sortSideTabs,
  StatusEditor,
  IconPicker,
  QuickStatus,
  StatusLine,
  SettingsSheet,
  type SettingsSection,
  AboutSheet,
  FeedbackSheet,
  FirstRunSheet,
  LicensesSheet,
  PrivacySheet,
  UninstallSheet,
  CommandsSection,
  playChime,
  type SortMode,
  terminalFontOptions,
  schemeTheme,
  linkActions,
  type TerminalLink,
  isLight,
  LIGHT_TINT,
  type Decorate,
  AskDialog,
} from "@terminescu/ui";
import type { Terminal } from "@xterm/xterm";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { flushSync } from "react-dom";
import { useStore } from "zustand";
import type { Answer, PaneApp, PaneReveal, Question, TextView } from "@terminescu/plugin-api";
import { usePlugins, type Plugins } from "./plugins";

export interface AppProps {
  windowId: string;
  host: Host;
  workspace: WorkspaceStore;
  /** Collects each pane's terminal, for Playwright on the mock host. */
  terminals?: Map<string, Terminal>;
}

/** Foreground processes that mean a terminal sits at its prompt, with nothing to lose. */
const LOGIN_SHELLS = new Set(["zsh", "bash", "fish", "sh", "dash", "tcsh", "csh", "ksh", "nu", "login"]);

/** What follows a merge a dragged tab started (task 8.70): the tab takes its place. */
type AfterMerge = { run(): void };
/** The undo toast: `undo` on its button, `done` when it goes without one. */
type Toast = { text: string; undo(): void; done?(): Promise<unknown> | void; removes?: Removal };
/** Something removed in the sidebars that waits on its undo toast before it is deleted. */
type Removal = { kind: "variable"; side: string; name: string } | { kind: "pin" | "command"; id: string };

/** Keeps a selection valid: the previous id while it still exists, else the first one. */
/**
 * A pane a plugin draws (D7, D9). With the plugin off the pane keeps its place and names the
 * plugin it needs.
 */
function PluginPaneView({
  kind,
  paneId,
  sideTabId,
  source,
  app,
  focused,
  plugins,
  openSettings,
}: {
  kind: string;
  paneId: string;
  sideTabId: string;
  source: string;
  app: PaneApp;
  focused: boolean;
  plugins: Plugins;
  openSettings(): void;
}) {
  const view = plugins.composed.paneView(kind);
  if (view) return <view.render paneId={paneId} sideTabId={sideTabId} source={source} app={app} focused={focused} bridge={plugins.bridge(view.plugin)} />;
  const owner = plugins.offOwner(kind);
  return (
    <div className="plugin-missing" role="status">
      <span>{owner ? `Needs the ${owner} plugin` : `Needs a plugin that shows ${kind} panes`}</span>
      <button type="button" className="ao-btn primary" onClick={openSettings}>
        Open Settings
      </button>
    </div>
  );
}

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
// The rows a window shows side by side (task 8.71), kept per window across restarts.
const viewKey = (windowId: string) => `terminescu.view.${windowId}`;
function loadView(windowId: string): View | undefined {
  try {
    const v = JSON.parse(localStorage.getItem(viewKey(windowId)) ?? "null") as View | null;
    return v && (v.type === "pane" || v.type === "split") ? v : undefined;
  } catch {
    return undefined;
  }
}
/** A view saved before task 8.75 holds row ids; each becomes that row's first tab. */
function savedTabs(v: View | undefined, sides: readonly SideTab[]): View | undefined {
  return rowsOf(v).reduce<View | undefined>((out, id) => {
    const tab = sides.find((s) => s.id === id)?.tabs[0];
    return out && tab ? replaceRow(out, id, tab.id) : out;
  }, v);
}
function saveView(windowId: string, v: View | undefined) {
  try {
    if (v && v.type === "split") localStorage.setItem(viewKey(windowId), JSON.stringify(v));
    else localStorage.removeItem(viewKey(windowId));
  } catch {
    // Storage can be off; the split then lasts until the window closes.
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

/** Whether a window point is inside the first element matching `selector`. */
function inside(selector: string, p: { x: number; y: number } | null): boolean {
  const r = p && document.querySelector(selector)?.getBoundingClientRect();
  return !!r && p.x >= r.left && p.x <= r.right && p.y >= r.top && p.y <= r.bottom;
}

/**
 * Sidebar targets under the pointer win; otherwise the nearest tab in the bar. Over the panes a
 * tab docks instead, and a pane dragged out finds its gap itself (task 8.32).
 */
const collide: CollisionDetection = (args) => {
  if (String(args.active.id).startsWith("pane:")) return [];
  if (inside(".app-pane", args.pointerCoordinates)) return [];
  const under = pointerWithin(args);
  // A side tab merges only into a row under the pointer.
  if (String(args.active.id).startsWith("side:")) return under.filter((c) => String(c.id).startsWith("into:"));
  if (under.length) return under;
  const tabs = args.droppableContainers.filter((c) => !String(c.id).includes(":"));
  return closestCenter({ ...args, droppableContainers: tabs });
};

/**
 * The drop target at a point, for a tab dragged from another window: the sidebar's end, a gap,
 * a side tab row, a tab listed in a folder (task 8.76), or a tab in the tab bar. Undefined
 * means the window itself.
 */
function remoteTarget(
  x: number,
  y: number,
):
  | { el: HTMLElement; kind: "end" }
  | { el: HTMLElement; kind: "gap"; index: number }
  | { el: HTMLElement; kind: "side" | "tab"; id: string }
  | { el: HTMLElement; kind: "kid"; id: string; folder: string }
  | undefined {
  const hit = document.elementFromPoint(x, y);
  const el = hit?.closest<HTMLElement>(".side-end, [data-gap], [data-kid], [data-side], .ao-tabs [data-tab]");
  if (!el) return undefined;
  if (el.classList.contains("side-end")) return { el, kind: "end" };
  if (el.dataset.gap !== undefined) return { el, kind: "gap", index: Number(el.dataset.gap) };
  if (el.dataset.kid) {
    const folder = el.closest(".side-item")?.querySelector<HTMLElement>("[data-side]")?.dataset.side;
    return folder ? { el, kind: "kid", id: el.dataset.kid, folder } : undefined;
  }
  if (el.dataset.side) return { el, kind: "side", id: el.dataset.side };
  return { el, kind: "tab", id: el.dataset.tab! };
}

const selectable = new Set(["palette", "send_to_tab"]);
/** Menu chords that a focused dialog field may use for itself. */
const FIELD_CHORDS = new Set(["save", "save_as", "return_edit", "send_to_pane", "send_to_tab"]);
/** Menu chords a focused text pane keeps for its editor: its own search and line jumps. */
const TEXT_CHORDS = new Set(["find", "find_next", "find_previous", "prompt_previous", "prompt_next", "select_all"]);
const paletteActions = (actions: readonly ActionDef[]) => actions.filter((a) => !selectable.has(a.id) && !/^(tab|side)_\d$/.test(a.id));

/**
 * Where a tab or row dragged over the panes would land (tasks 8.32, 8.75): beside the shown
 * tab `area`, the one holding `pane`; `small` means it would not fit.
 */
interface Dock {
  pane: string;
  area: string;
  side: DockSide | "center";
  box: Box;
  small: boolean;
}
const sameDock = (a: Dock | undefined, b: Dock | undefined) =>
  a === b ||
  (!!a && !!b && a.pane === b.pane && a.area === b.area && a.side === b.side && a.small === b.small && JSON.stringify(a.box) === JSON.stringify(b.box));

/** Where a pane dragged to the tab bar becomes a tab: before the tab at `index`. */
interface TabGap {
  index: number;
  left: number;
  top: number;
  height: number;
}

/** The gap in the tab bar nearest a window point, or undefined off the bar. */
function tabGapAt(x: number, y: number): TabGap | undefined {
  const bar = document.querySelector<HTMLElement>(".app-main .ao-tabs");
  const r = bar?.getBoundingClientRect();
  if (!bar || !r || x < r.left || x > r.right || y < r.top || y > r.bottom) return undefined;
  const tabs = [...bar.querySelectorAll<HTMLElement>(":scope > [data-tab]")].map((el) => el.getBoundingClientRect());
  const index = tabs.filter((t) => t.left + t.width / 2 < x).length;
  const left = index < tabs.length ? tabs[index]!.left : (tabs.at(-1)?.right ?? r.left);
  return { index, left, top: r.top, height: r.height };
}

/** A split pane's header; dragged to the tab bar, its pane becomes a tab (task 8.32). */
function PaneHead({ paneId, name, children }: { paneId: string; name: string; children: React.ReactNode }) {
  const { attributes, listeners, setNodeRef } = useDraggable({ id: `pane:${paneId}` });
  return (
    <div ref={setNodeRef} {...attributes} {...listeners} tabIndex={-1} className="ao-tp-head" title={`${name}\nDrag to the tab bar to make it a tab`}>
      {children}
    </div>
  );
}

export function App({ windowId, host, workspace, terminals }: AppProps) {
  const ws = useStore(workspace.store, (s) => s.workspace);
  const win = ws.windows.find((w) => w.id === windowId);
  // Claude sessions by pane, from hook events in the core (D34).
  const [agents, setAgents] = useState<Agents>({});
  // Each terminal pane's path, program, process and Claude title, for tab names and subtitles (tasks 8.30, 8.62).
  const [labels, setLabels] = useState<Labels>({});
  const [focusByTab, setFocusByTab] = useState<Record<string, string>>({});
  // Each tab carries its focused terminal's live name, `zsh` or `npm`, for `tabName` (task 8.62).
  const sideTabs = useMemo(() => withLiveNames(win?.sideTabs ?? [], focusByTab, labels, agents), [win?.sideTabs, focusByTab, labels, agents]);

  // Shown statuses and the saved config, from the core (D15).
  const [statuses, setStatuses] = useState<Record<string, Shown>>({});
  const [statusConfig, setStatusConfig] = useState<StatusConfig>({ statuses: [], rules: [], builtins: {} });
  const [sort, setSort] = useState<SortMode>(loadSort);
  // The sidebar order; ⌃⌘1 to ⌃⌘9 follow what the person sees.
  const allTabs = useMemo(() => sideTabs.flatMap((s) => s.tabs), [sideTabs]);
  const shownSides = sortSideTabs(sideTabs, statuses, sort, sideTabName);
  const changeSort = (mode: SortMode) => {
    setSort(mode);
    saveSort(mode);
  };

  // Task 8.75: the tabs shown side by side, saved per window. A window opens on the first one.
  const [view, setView] = useState<View | undefined>(() => savedTabs(loadView(windowId), win?.sideTabs ?? []));
  const firstShown = () => {
    const tab = rowsOf(view)[0];
    const side = win?.sideTabs.find((s) => s.tabs.some((t) => t.id === tab));
    return side && tab ? { side: side.id, tab } : undefined;
  };
  const [sideTabId, setSideTabId] = useState<string | undefined>(() => firstShown()?.side);
  const activeSide = sideTabs.find((s) => s.id === pick(sideTabs.map((s) => s.id), sideTabId));
  const tabs = activeSide?.tabs ?? [];
  const [tabBySide, setTabBySide] = useState<Record<string, string>>(() => {
    const f = firstShown();
    return f ? { [f.side]: f.tab } : {};
  });
  const activeTab = tabs.find((t) => t.id === pick(tabs.map((t) => t.id), activeSide && tabBySide[activeSide.id]));
  const panes = activeTab ? panesOf(activeTab.layout) : [];
  const focused = pick(panes, activeTab && focusByTab[activeTab.id]);
  // The selected tab is always shown; a tab picked in the sidebar or the tab bar takes the
  // place of the one selected before.
  const prevTab = useRef<{ tab: string; side: string } | undefined>(undefined);
  const tabIds = allTabs.map((t) => t.id);
  const shownView = tabIds.length ? fitView(view, tabIds, activeTab?.id, prevTab.current?.tab) : view;
  const openTabs = rowsOf(shownView);
  useEffect(() => {
    // The selected tab closed with its row: the next shown tab takes the selection, not its
    // place. One closed in a folder leaves its place to the folder's next tab.
    const prev = prevTab.current;
    if (prev && !tabIds.includes(prev.tab) && activeSide?.id !== prev.side && activeTab && !rowsOf(view).includes(activeTab.id)) {
      const left = rowsOf(fitView(view, tabIds, undefined, undefined))[0];
      const side = left && sideTabs.find((s) => s.tabs.some((t) => t.id === left));
      if (side && left) {
        selectTab(side.id, left);
        return;
      }
    }
    prevTab.current = activeTab && activeSide && { tab: activeTab.id, side: activeSide.id };
    if (tabIds.length && JSON.stringify(shownView) !== JSON.stringify(view)) {
      setView(shownView);
      saveView(windowId, shownView);
    }
  });
  // The pixel tooltip under an area's head (task 8.73); it goes when the areas change, so it
  // never stays over a head that left.
  const areaTip = useTip();
  const openKey = openTabs.join(" ");
  useEffect(() => areaTip.hide(), [openKey]);
  const changeView = (f: (v: View | undefined) => View | undefined) =>
    setView((v) => {
      const next = f(v);
      saveView(windowId, next);
      return next;
    });
  // Each shown tab with its row.
  const areas = openTabs.flatMap((id) => {
    const side = sideTabs.find((s) => s.tabs.some((t) => t.id === id));
    const tab = side?.tabs.find((t) => t.id === id);
    return side && tab ? [{ side, tab }] : [];
  });
  const areaOfPane = (paneId: string) => areas.find((a) => panesOf(a.tab.layout).includes(paneId));
  /** A shown tab's name: a folder's tab as "Folder › tab". */
  const areaName = (x: { side: SideTab; tab: Tab }) => (x.side.tabs.length > 1 ? `${sideTabName(x.side, docs)} › ${tabName(x.tab, docs)}` : sideTabName(x.side, docs));
  // The pane each tab shows alone (task 8.39), for this session only; a pane that left the tab
  // shows the whole layout again.
  const [maxByTab, setMaxByTab] = useState<Record<string, string>>({});
  const maxPane = activeTab && panes.length > 1 && panes.includes(maxByTab[activeTab.id]!) ? maxByTab[activeTab.id] : undefined;
  const restore = (tabId: string) => setMaxByTab((m) => (tabId in m ? Object.fromEntries(Object.entries(m).filter(([t]) => t !== tabId)) : m));
  // A maximized pane that left its tab, or a tab down to one pane, forgets it, so a pane added
  // later by any route shows beside the others.
  const staleMax = activeTab && activeTab.id in maxByTab && !maxPane ? activeTab.id : undefined;
  useEffect(() => {
    if (staleMax) restore(staleMax);
  }, [staleMax]);

  const [error, setError] = useState<string>();
  // A verified update waiting for Restart to update (add-public-release D4).
  const [update, setUpdate] = useState<string>();
  useEffect(() => {
    let gone = false;
    let off: (() => void) | undefined;
    void host.onUpdate((v) => !gone && setUpdate(v)).then((o) => (gone ? o() : (off = o)));
    return () => {
      gone = true;
      off?.();
    };
  }, [host]);
  const [palette, setPalette] = useState(false);
  const [mode, setMode] = useState<ContextMode>(loadMode);
  const [dragging, setDragging] = useState<string>();
  // A side tab merge waiting on the clash dialog (D5).
  const [openFolder, setOpenFolder] = useState<{ id: string }>();
  const [merging, setMerging] = useState<{ from: SideTab; into: SideTab; clashes: MergeClash[]; after?: AfterMerge }>();
  // The undo toast for the last merge or removed variable. `done` runs once the toast goes
  // without Undo: a removed variable is deleted only then.
  const [toast, setToast] = useState<Toast>();
  const toastRef = useRef<Toast>(undefined);
  const endToast = (undo: boolean) => {
    const t = toastRef.current;
    toastRef.current = undefined;
    setToast(undefined);
    if (undo) t?.undo();
    else return t?.done?.();
  };
  const showToast = (next: Toast) => {
    toastRef.current?.done?.();
    toastRef.current = next;
    setToast(next);
  };
  // Tab stays inside an open dialog, and focus comes back when it closes.
  useEffect(() => {
    const root = document.getElementById("root");
    return root ? trapModalFocus(root) : undefined;
  }, []);
  // A window that closes while a toast shows keeps what the toast would have kept.
  useEffect(() => {
    const flush = () => endToast(false);
    window.addEventListener("pagehide", flush);
    return () => window.removeEventListener("pagehide", flush);
  }, []);
  // Variables, pins and commands removed but kept until their toast goes.
  const [removing, setRemoving] = useState<Removal[]>([]);
  const [sheet, setSheet] = useState<
    | "status"
    | "settings"
    | "prompts"
    | "insert"
    | "save-prompt"
    | "first-run"
    | "about"
    | "licenses"
    | "privacy"
    | "feedback-email"
    | "feedback-issue"
    | "uninstall"
  >();
  // The first-run hook tick, kept here so a look at Privacy does not reset it.
  const [firstRunHook, setFirstRunHook] = useState(true);
  const [toSave, setToSave] = useState("");
  // Documents a plugin's pane view draws, as a diff tab: the text pane under a plugin's
  // decoration. For this window's life; a view whose plugin is off shows plain text.
  const [docViews, setDocViews] = useState<Record<string, TextView>>({});
  const [libraryKey, setLibraryKey] = useState<string>();
  // Text panes' documents as their panes last reported them: titles and unsaved edits (D27).
  const [docs, setDocs] = useState<Record<string, DocView>>({});
  const noteDoc = (d: DocView) => setDocs((m) => ({ ...m, [d.id]: d }));
  // `⌘O`: open while `finder` is set; `list` arrives after (D27).
  const [finder, setFinder] = useState<{ list?: FileList }>();
  const [findingAll, setFindingAll] = useState(false);
  // The terminal pane whose find bar is open (task 8.38); one per window.
  const [findPane, setFindPane] = useState<string>();
  const searchBar = useRef<SearchBarHandle>(null);

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
  // A pane of another shown tab selects that tab (tasks 8.71, 8.75).
  const focusArea = (tabId: string, paneId: string) => {
    const side = sideTabs.find((s) => s.tabs.some((t) => t.id === tabId));
    if (side && tabId !== activeTab?.id) selectTab(side.id, tabId);
    focusIn(tabId, paneId);
  };
  // The terminal each tab focused last: where "Send to pane" pastes (D29).
  const lastTerminal = useRef<Record<string, string>>({});
  useEffect(() => {
    if (activeTab && focused && terminalsOf(activeTab.layout).includes(focused)) lastTerminal.current[activeTab.id] = focused;
  }, [activeTab, focused]);

  // The shown tab's shell, for the panel's plugin sections.
  const shellTerms = activeTab ? terminalsOf(activeTab.layout) : [];
  const lastShell = activeTab && lastTerminal.current[activeTab.id];
  const shellPane = focused && shellTerms.includes(focused) ? focused : lastShell && shellTerms.includes(lastShell) ? lastShell : shellTerms[0];

  // What a command created gets selected once its patch arrives: a check that runs on every
  // workspace change until it finds its target.
  const follow = useRef<((w: Workspace) => boolean) | undefined>(undefined);
  // A check gives up after FOLLOW_MS, so a late unrelated change never jumps the selection.
  const setFollow = (check: (w: Workspace) => boolean) => {
    const until = Date.now() + FOLLOW_MS;
    follow.current = (w) => Date.now() > until || check(w);
  };
  const wsNow = useRef(ws);
  wsNow.current = ws;
  const sidesOf = (w: Workspace) => w.windows.find((x) => x.id === windowId)?.sideTabs ?? [];
  const followNewSide = () => {
    const before = new Set(sideTabs.map((s) => s.id));
    setFollow((w) => {
      const added = sidesOf(w).find((s) => !before.has(s.id));
      if (added) setSideTabId(added.id);
      return !!added;
    });
  };
  // The sidebar's + ▾ › Text (task 8.59): a new side tab that holds an untitled text. The side
  // tab starts with a shell, which closes once the text is in.
  const newTextSide = (index?: number) => {
    const before = new Set(sideTabs.map((s) => s.id));
    setFollow((w) => {
      const added = sidesOf(w).find((s) => !before.has(s.id));
      if (!added) return false;
      setSideTabId(added.id);
      const shells = added.tabs.map((t) => t.id);
      void host.newText(added.id).then(
        (d) => {
          revealDoc(d.id);
          for (const tabId of shells) void send({ type: "CloseTab", tabId });
        },
        (e: unknown) => setError(String(e)),
      );
      return true;
    });
    void send({ type: "CreateSideTab", windowId, index });
  };
  // ⌥⌘T and the sidebar's + ▾ › Text follow ⌘T (task 8.80): into the selected folder, or a
  // row of its own right below a one-tab row.
  const newTextBeside = () => {
    // An emptied last window has no selected row: the text opens in a row of its own.
    if (!activeSide) return newTextSide();
    if (activeSide.tabs.length > 1) return void host.newText(activeSide.id).then((d) => revealDoc(d.id), setError);
    newTextSide(sideTabs.findIndex((s) => s.id === activeSide.id) + 1);
  };
  const followTab = (tabId: string) => {
    setFollow((w) => {
      const side = sidesOf(w).find((s) => s.tabs.some((t) => t.id === tabId));
      if (side) selectTab(side.id, tabId);
      return !!side;
    });
  };
  const followNewTab = (sideId: string) => {
    const before = new Set(sideTabs.flatMap((sd) => sd.tabs).map((t) => t.id));
    setFollow((w) => {
      const added = sidesOf(w)
        .find((s) => s.id === sideId)
        ?.tabs.find((t) => !before.has(t.id));
      if (added) selectTab(sideId, added.id);
      return !!added;
    });
  };
  // A new terminal sits at the selected row's level (task 8.79): inside its folder, or as a
  // row of its own right below a one-tab row. Only a drag makes a folder.
  const newBeside = () => {
    if (!activeSide) {
      followNewSide();
      return void send({ type: "CreateSideTab", windowId });
    }
    if (activeSide.tabs.length > 1) {
      followNewTab(activeSide.id);
      return void send({ type: "CreateTab", sideTabId: activeSide.id });
    }
    followNewSide();
    void send({ type: "CreateSideTab", windowId, index: sideTabs.findIndex((s) => s.id === activeSide.id) + 1 });
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

  // Shows the pane that edits a document, now or once the core's patch adds it.
  const revealDoc = (docId: string) => {
    const pane = (w: Workspace) =>
      sidesOf(w)
        .flatMap((s) => s.tabs)
        .flatMap((t) => panesOf(t.layout))
        .find((p) => sidesOf(w).some((s) => s.tabs.some((t) => docOf(t.layout, p) === docId)));
    const found = pane(workspace.store.getState().workspace);
    if (found) return revealRef.current(found);
    setFollow((w) => {
      const p = pane(w);
      if (p) revealRef.current(p);
      return !!p;
    });
  };

  // Saves a text pane: its last keystrokes go first, an Untitled buffer or Save As asks where.
  // False when the person cancelled the dialog or the save failed.
  const saveDoc = async (paneId: string, as: boolean): Promise<boolean> => {
    const pane = textPane(paneId);
    if (!pane) return false;
    try {
      await pane.flush();
      const doc = await host.document(pane.docId);
      let path: string | null | undefined;
      if (as || doc.source.kind === "untitled") {
        if (doc.sessionOnly) throw new Error(`${doc.title} is session only and never goes to disk.`);
        path = await host.chooseSavePath(doc.source.kind === "file" ? doc.source.path : doc.title);
        if (!path) return false;
      }
      noteDoc(await host.saveDocument(pane.docId, path));
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    }
  };
  // Opens a file in the active side tab, or beside the focused pane, and shows it.
  const openDoc = (path: string, beside: boolean) => {
    if (!activeSide) return;
    void host.openFile(activeSide.id, path, beside ? (focused ?? null) : null).then((d) => revealDoc(d.id), (e: unknown) => setError(String(e)));
  };
  // A panel section opens a file beside the focused pane, as a plugin's view when it names one.
  const openText = (path: string, view?: TextView): Promise<void> => {
    if (!activeSide) return Promise.reject(new Error("Select a row first."));
    return host.openFile(activeSide.id, path, focused ?? null).then((d) => {
      if (view) setDocViews((v) => ({ ...v, [d.id]: view }));
      revealDoc(d.id);
    });
  };
  const browse = (dir?: string) => void host.chooseFile(dir).then((p) => p && openDoc(p, false), (e: unknown) => setError(String(e)));
  // `⌘O` lists files under the focused terminal's cwd; `⌘O` again goes to the Open dialog.
  const openFinder = () => {
    if (finder) {
      setFinder(undefined);
      return browse(finder.list?.root);
    }
    const terms = activeTab ? terminalsOf(activeTab.layout) : [];
    const pane = focused && terms.includes(focused) ? focused : (terms[0] ?? null);
    setFinder({});
    void host.listFiles(pane).then(
      (list) => setFinder((f) => f && { list }),
      (e: unknown) => {
        setFinder(undefined);
        setError(String(e));
      },
    );
  };
  // `⌘↵` in a text pane a program waits on: save, let `terminescu edit --wait` return, close.
  const returnEdit = async () => {
    const pane = focused && textPane(focused);
    if (!focused || !pane) return;
    const doc = await host.document(pane.docId).catch(() => undefined);
    if (!doc?.request) return setNotice("No program waits on this text pane.");
    if (!(await saveDoc(focused, false))) return;
    await host.returnEdit(doc.id).catch((e: unknown) => setError(String(e)));
    void send({ type: "ClosePane", paneId: focused });
  };
  // `⌘⇧↵` in a text pane: the selection, or all of it, goes to a terminal as a paste without
  // Enter. The target is the tab's last focused terminal, else one running Claude, else the
  // first; then the other tabs of the side tab.
  // The terminal a tab's text goes to: the one last focused, else a Claude pane, else the first.
  const pickIn = (t: Tab) => {
    const terms = terminalsOf(t.layout);
    const last = lastTerminal.current[t.id];
    return (last && terms.includes(last) ? last : undefined) ?? terms.find((p) => agents[p]) ?? terms[0];
  };
  // "Send to Tab…" (D13): the focused pane's selection, then a tab to paste it into.
  const [sending, setSending] = useState<string>();
  const sendToTab = () => {
    if (!focused || !activeTab) return;
    const text = textPane(focused)
      ? textPane(focused)!.selectionOrAll()
      : transcriptOf(activeTab.layout, focused)
        ? (window.getSelection()?.toString() ?? "")
        : terminalSelection(focused);
    if (!text.trim()) return setError("Select text to send to another tab.");
    setSending(text);
  };
  const sendItems: PaletteItem[] = sending
    ? sideTabs.flatMap((sd) =>
        sd.tabs
          .filter((t) => t.id !== activeTab?.id && pickIn(t))
          .map((t) => ({
            id: `send:${t.id}`,
            label: `${sideTabName(sd)} › ${tabName(t)}`,
            group: "Send to tab",
            run: () => void host.sendText(pickIn(t)!, sending).catch((e: unknown) => setError(String(e))),
          })),
      )
    : [];
  const sendToPane = () => {
    const pane = focused && textPane(focused);
    if (!pane || !activeSide || !activeTab) return setError("Focus a text pane to send its text.");
    const text = pane.selectionOrAll();
    if (!text.trim()) return;
    const target = [activeTab, ...activeSide.tabs.filter((t) => t.id !== activeTab.id)].map(pickIn).find(Boolean);
    if (!target) return setError("No terminal pane in this side tab to send to.");
    paneEvent(target, new CustomEvent(INSERT_EVENT, { detail: { text, stay: true } }));
  };
  const saveFocused = (as: boolean) => {
    if (!focused || !textPane(focused)) return setError("Focus a text pane to save.");
    void saveDoc(focused, as);
  };

  // Closing panes with unsaved text asks first: Save, Don't save or Cancel (D27).
  const [closing, setClosing] = useState<{ panes: string[]; titles: string[]; command: Command; all: string[] }>();
  // Closing a terminal that still runs something asks too, as Terminal.app does: the program
  // ends with its shell.
  const [closingRunning, setClosingRunning] = useState<{ names: string[]; command: Command }>();
  const runningIn = (pane: string): string | undefined => {
    const agent = agents[pane];
    if (agent && agent.state !== "done") return "claude";
    const label = labels[pane];
    if (label?.program) return label.program;
    return label?.process && !LOGIN_SHELLS.has(label.process.replace(/^-/, "")) ? label.process : undefined;
  };
  // The row a close command ends: the row itself, or the row whose only tab goes, which is
  // also the case when its last pane holding a shell or a text closes (as the core plans it).
  const rowClosedBy = (command: Command): string | undefined => {
    if (command.type === "CloseSideTab") return command.sideTabId;
    const tabId =
      command.type === "CloseTab"
        ? command.tabId
        : command.type === "ClosePane"
          ? sideTabs.flatMap((sd) => sd.tabs).find((t) => panesOf(t.layout).includes(command.paneId) && !worksBesides(t.layout, command.paneId))?.id
          : undefined;
    const side = tabId ? sideTabs.find((sd) => sd.tabs.some((t) => t.id === tabId)) : undefined;
    return side && side.tabs.length === 1 ? side.id : undefined;
  };
  // A row's close runs the plugins' close hooks last, after every question of the app's own,
  // so a Cancel there cannot leave a row whose worktree a hook removed (add-git-plugin 3.3).
  // Any hook may keep the row; a throw keeps it too and shows why. A second close of a row
  // whose hooks still ask is ignored.
  const rowsClosing = useRef(new Set<string>());
  const sendClose = async (command: Command) => {
    const row = rowClosedBy(command);
    if (row && plugins.composed.beforeRowClose.length) {
      if (rowsClosing.current.has(row)) return;
      rowsClosing.current.add(row);
      try {
        const side = sideTabs.find((sd) => sd.id === row);
        const title = side ? sideTabName(side, docs) : "the side tab";
        for (const hook of plugins.composed.beforeRowClose) {
          let go: boolean;
          try {
            go = await hook.run({ sideTabId: row, title, bridge: plugins.bridge(hook.plugin), ask });
          } catch (e) {
            setError(`Close ${title}: ${e instanceof Error ? e.message : String(e)}`);
            go = false;
          }
          if (!go) return refocus();
        }
        await send(command).catch((e: unknown) => setError(`Close ${title}: ${e instanceof Error ? e.message : String(e)}`));
      } finally {
        rowsClosing.current.delete(row);
      }
      return;
    }
    await send(command);
  };
  const closeIfIdle = (panes: string[], command: Command) => {
    const names = [...new Set(panes.map(runningIn).filter((n): n is string => !!n))];
    if (!names.length) return void sendClose(command);
    setClosingRunning({ names, command });
  };
  const closeWith = async (panes: string[], command: Command) => {
    const dirty: { pane: string; title: string }[] = [];
    for (const p of panes) {
      const t = textPane(p);
      if (!t) continue;
      await t.flush();
      const d = await host.document(t.docId).catch(() => undefined);
      if (d?.dirty) dirty.push({ pane: p, title: d.title });
    }
    if (!dirty.length) return closeIfIdle(panes, command);
    setClosing({ panes: dirty.map((d) => d.pane), titles: dirty.map((d) => d.title), command, all: panes });
  };
  const closePane = (paneId: string) => void closeWith([paneId], { type: "ClosePane", paneId });
  const closeTab = (tabId: string) => {
    const tab = sideTabs.flatMap((sd) => sd.tabs).find((t) => t.id === tabId);
    void closeWith(tab ? panesOf(tab.layout) : [], { type: "CloseTab", tabId });
  };
  const finishClose = async (choice: "save" | "discard" | "cancel") => {
    const c = closing;
    setClosing(undefined);
    if (!c || choice === "cancel") return refocus();
    if (choice === "save") {
      for (const p of c.panes) if (!(await saveDoc(p, false))) return refocus();
    }
    closeIfIdle(c.all, c.command);
  };
  const finishCloseRunning = (close: boolean) => {
    const c = closingRunning;
    setClosingRunning(undefined);
    if (c && close) void sendClose(c.command);
    else refocus();
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
    // Documents whose panes closed leave the marks.
    const shown = new Set(sidesOf(ws).flatMap((sd) => sd.tabs.flatMap((t) => panesOf(t.layout).flatMap((p) => docOf(t.layout, p) ?? []))));
    setDocs((m) => (Object.keys(m).every((d) => shown.has(d)) ? m : Object.fromEntries(Object.entries(m).filter(([d]) => shown.has(d)))));
  }, [ws]);

  // The menu takes ⌘S and ⌘↵ before the page sees them. Outside a text pane they belong to
  // the focused field, as a dialog's ⌘↵ and the prompt library's ⌘S, so they go back there.
  const passToField = (id: string) => {
    const el = document.activeElement;
    if (!(el instanceof HTMLElement) || el.closest(".text-pane")) return false;
    if (!el.closest('[role="dialog"]') && !el.matches("input, textarea, select")) return false;
    const accel = withBindings(ACTIONS, keyBindings).find((a) => a.id === id)?.accel;
    if (!accel) return false;
    el.dispatchEvent(new KeyboardEvent("keydown", chordEvent(accel)));
    return true;
  };

  // ⌘F and ⌘G belong to the editor in a text pane: they go back to it as passToField does.
  const passToText = (id: string) => {
    const el = document.activeElement;
    if (!(el instanceof HTMLElement) || !el.closest(".text-pane")) return false;
    const accel = withBindings(ACTIONS, keyBindings).find((a) => a.id === id)?.accel;
    if (!accel) return false;
    el.dispatchEvent(new KeyboardEvent("keydown", chordEvent(accel)));
    return true;
  };

  // A text field outside the panes keeps ⌘A and, outside the find bar, ⌘↑ and ⌘↓ as caret
  // moves, while ⌘F and ⌘G do nothing there: the menu takes the chords, so they act here.
  const keepForField = (id: string) => {
    const el = document.activeElement;
    if (!(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement)) return false;
    if (el.closest(".text-pane, .term-pane")) return false;
    if (id === "select_all") {
      el.select();
      return true;
    }
    if (el.closest(".find-bar")) return false;
    if (id === "prompt_previous" || id === "prompt_next") {
      const at = id === "prompt_previous" ? 0 : el.value.length;
      try {
        el.setSelectionRange(at, at);
      } catch {
        // number and range inputs have no caret
      }
    }
    return true;
  };

  const act = (id: string) => {
    if (FIELD_CHORDS.has(id) && passToField(id)) return;
    if (TEXT_CHORDS.has(id) && passToText(id)) return;
    if (TEXT_CHORDS.has(id) && keepForField(id)) return;
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
    // A plugin action reads `plugin.action`; app actions never hold a dot.
    if (id.includes(".")) return runPluginAction(id);
    switch (id) {
      case "new_tab":
        return newBeside();
      // ⌘N opens a window with one new tab (task 8.81).
      case "new_window":
        return void send({ type: "CreateWindow" });
      // Every other window's rows join this one (task 8.84).
      case "merge_windows":
        if (ws.windows.length < 2) return;
        return void send({ type: "MergeAllWindows", windowId }).then(refocus, report);
      case "new_side_tab":
        followNewSide();
        return void send({ type: "CreateSideTab", windowId });
      case "split_right":
      case "split_down":
        if (!activeTab || !focused) return;
        restore(activeTab.id);
        followNewPane(activeTab.id);
        return void send({ type: "SplitPane", paneId: focused, dir: id === "split_right" ? "row" : "column" });
      case "close":
        if (focused) closePane(focused);
        return;
      // The last closed tab or row comes back where it was, and shows (task 8.87).
      case "reopen_closed":
        return void host.reopenClosed(windowId).then((r) => {
          // Back in another window: that one comes forward and shows it.
          if (!r || r.windowId !== windowId) return refocus();
          const pick = (w: Workspace) => {
            const ok = sidesOf(w).some((s) => s.id === r.sideTabId && s.tabs.some((t) => t.id === r.tabId));
            if (ok) {
              selectTab(r.sideTabId, r.tabId);
              refocus();
            }
            return ok;
          };
          if (!pick(wsNow.current)) setFollow(pick);
        }, report);
      case "maximize_pane":
        if (!activeTab || !focused || panes.length < 2) return;
        if (maxPane) restore(activeTab.id);
        else setMaxByTab((m) => ({ ...m, [activeTab.id]: focused }));
        return refocus();
      case "pane_to_tab":
        // A tab's only pane is already a tab.
        if (!activeTab || !focused || activeTab.layout.type !== "split") return;
        return paneToTab(focused, tabs.indexOf(activeTab) + 1);
      case "new_text":
        return newTextBeside();
      case "save":
      case "save_as":
        return void saveFocused(id === "save_as");
      case "open_file":
        return openFinder();
      case "return_edit":
        return void returnEdit();
      case "send_to_pane":
        return sendToPane();
      case "send_to_tab":
        return sendToTab();
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
      case "zoom_in":
      case "zoom_out":
      case "zoom_reset":
        return void zoom(id === "zoom_reset" ? 0 : id === "zoom_in" ? 1 : -1);
      case "palette":
        return setPalette((p) => !p);
      case "status_editor":
        setStatusFor("side");
        if (activeSide) setSheet((s) => (s === "status" ? undefined : "status"));
        return;
      case "settings":
        return setSheet((s) => (s === "settings" ? undefined : "settings"));
      // The Help menu and About (add-public-release D7, D9): chordless, so not in actions.json.
      case "about":
      case "licenses":
      case "privacy":
      case "uninstall":
        return setSheet(id as "about" | "licenses" | "privacy" | "uninstall");
      case "check_updates":
        return void host.checkUpdates().then(
          (v) => (v ? setUpdate(v) : setNotice("Terminescu is up to date.")),
          (e: unknown) => setError(String(e)),
        );
      case "feedback_email":
        return setSheet("feedback-email");
      case "feedback_issue":
        return setSheet("feedback-issue");
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
      case "find":
        if (!focused || !activeTab || !terminalsOf(activeTab.layout).includes(focused)) return;
        if (findPane === focused) searchBar.current?.focus();
        else setFindPane(focused);
        return;
      case "find_all":
        setFindingAll(true);
        return;
      case "prompt_previous":
      case "prompt_next":
      case "select_output":
        if (focused) promptAction(focused, id === "select_output" ? "output" : id === "prompt_previous" ? "previous" : "next");
        return;
      case "select_all":
        if (focused) selectAllIn(focused);
        return;
      case "clear_to_start":
      case "clear_scrollback":
        if (focused) clearTerminal(focused, id === "clear_scrollback");
        return;
      case "find_next":
      case "find_previous":
        if (findPane && findPane === focused) searchBar.current?.next(id === "find_previous");
        return;
      case "focus_left":
      case "focus_right":
      case "focus_up":
      case "focus_down": {
        // The neighbour comes from the panes on screen, so a maximized pane shows the split first.
        if (maxPane && activeTab) flushSync(() => restore(activeTab.id));
        const next = focused && neighbor(focused, panes, id.slice(6) as Dir);
        if (next && activeTab) focusIn(activeTab.id, next);
        return;
      }
    }
  };
  const subtitles = useMemo(() => tabSubtitles(sideTabs.flatMap((s) => s.tabs), focusByTab, labels, agents), [sideTabs, focusByTab, labels, agents]);
  // What runs in each tab: its icons blink, and bounce while Claude waits (task 8.54).
  const activity = useMemo(() => tabActivities(sideTabs.flatMap((s) => s.tabs), labels, agents), [sideTabs, labels, agents]);
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
    let heardLabels = false;
    void host
      .onLabels((m) => {
        heardLabels = true;
        if (!gone) setLabels(m);
      })
      .then(keep);
    void host.labels().then((m) => !gone && !heardLabels && setLabels(m));
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
  type Ctx = { side?: string; variables: Variable[]; items: Item[]; pins: Pin[]; files: TouchedFile[] };
  const [ctx, setCtx] = useState<Ctx>({ variables: [], items: [], pins: [], files: [] });
  const sideId = activeSide?.id;
  useEffect(() => {
    if (!sideId) return;
    let gone = false;
    let off: (() => void) | undefined;
    const load = () =>
      void Promise.all([host.variables(sideId), host.items(sideId), host.pins(sideId), host.touchedFiles(sideId)])
        .then(([variables, items, pins, files]) => !gone && setCtx({ side: sideId, variables, items, pins, files }))
        .catch((e: unknown) => !gone && setError(String(e)));
    load();
    void host.onContext((s) => s === sideId && load()).then((o) => (gone ? o() : (off = o)));
    return () => {
      gone = true;
      off?.();
    };
  }, [host, sideId]);
  const shownCtx: Ctx = useMemo(() => {
    if (ctx.side !== sideId) return { variables: [], items: [], pins: [], files: [] };
    if (!removing.length) return ctx;
    const variable = (name: string) => removing.some((r) => r.kind === "variable" && r.side === sideId && r.name === name);
    const pin = (id: string) => removing.some((r) => r.kind === "pin" && r.id === id);
    return { ...ctx, variables: ctx.variables.filter((v) => !variable(v.name)), pins: ctx.pins.filter((p) => !pin(p.id)) };
  }, [ctx, sideId, removing]);
  // Names and modes for the editor's chips; values stay out of the text panes (D35).
  const chipVars = useMemo(
    () => (ctx.side === sideId ? ctx.variables.map((v) => ({ name: v.name, mode: v.mode })) : undefined),
    [ctx, sideId],
  );

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
  const [reveal, setReveal] = useState<PaneReveal & { session: string }>();
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
  // Settings › Variables: what Extract starts with.
  const [appSettings, setAppSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  useEffect(() => {
    let gone = false;
    let off: (() => void) | undefined;
    let asked = false;
    const load = () =>
      void host.settings().then((s) => {
        if (gone) return;
        setAppSettings(s);
        // The first-run screen opens once, from the stored settings, never from the defaults.
        if (!asked && !s.firstRunDone) setSheet("first-run");
        asked = true;
      }, () => undefined);
    load();
    void host.onSettings(load).then((o) => (gone ? o() : (off = o)));
    return () => {
      gone = true;
      off?.();
    };
  }, [host]);
  // The title row keeps its size on screen at any zoom, like a native title bar: the traffic
  // lights don't zoom, so the strip beside them must not either.
  useEffect(() => {
    document.documentElement.style.setProperty("--chrome", String(100 / appSettings.zoomPercent));
  }, [appSettings.zoomPercent]);
  // Task 8.36: Settings › Terminal sets the tab text size and every terminal's font, live.
  useEffect(() => {
    document.documentElement.style.setProperty("--tab-font", `${appSettings.tabFontSize}px`);
  }, [appSettings.tabFontSize]);
  const { terminalFont, terminalFontSize, terminalLineHeight, terminalLetterSpacing, terminalFontWeight } = appSettings;
  const termFont = useMemo(
    () => terminalFontOptions({ terminalFont, terminalFontSize, terminalLineHeight, terminalLetterSpacing, terminalFontWeight }),
    [terminalFont, terminalFontSize, terminalLineHeight, terminalLetterSpacing, terminalFontWeight],
  );
  // Task 8.42: cursor, history, Option keys, bell, clipboard and contrast, live.
  const { terminalCursor, terminalCursorBlink, terminalScrollback, optionAsMeta, bell, copyOnSelect, pasteWarning, raiseContrast, terminalKeys } = appSettings;
  const termBehaviour = useMemo(
    () => ({ terminalCursor, terminalCursorBlink, terminalScrollback, optionAsMeta, bell, copyOnSelect, pasteWarning, raiseContrast, terminalKeys }),
    [terminalCursor, terminalCursorBlink, terminalScrollback, optionAsMeta, bell, copyOnSelect, pasteWarning, raiseContrast, terminalKeys],
  );
  // Task 8.47: the colour scheme and opacity. The page paints the panes' margins in the scheme's
  // background; below 100% the window lets the desktop through there, the chrome stays solid.
  const { terminalTheme, terminalColors, terminalOpacity } = appSettings;
  const scheme = useMemo(() => schemeTheme(terminalTheme, terminalColors), [terminalTheme, terminalColors]);
  useEffect(() => {
    const root = document.documentElement;
    const custom = terminalTheme !== "terminescu" || terminalOpacity < 100;
    if (custom) root.dataset.scheme = "";
    else delete root.dataset.scheme;
    if (terminalOpacity < 100) root.dataset.seeThrough = "";
    else delete root.dataset.seeThrough;
    root.style.setProperty("--term-base", scheme.background ?? "");
    root.style.setProperty("--term-alpha", `${terminalOpacity}%`);
    root.style.setProperty("--tint-mix", isLight(scheme.background ?? "") ? `${LIGHT_TINT * 100}%` : "100%");
  }, [scheme, terminalTheme, terminalOpacity]);
  // A risky paste waits on this dialog; one at a time.
  const [pasteAsk, setPasteAsk] = useState<{ text: string; reason: string; answer(ok: boolean): void }>();
  const confirmPaste = (text: string, reason: string) =>
    new Promise<boolean>((resolve) => {
      setPasteAsk((prev) => {
        prev?.answer(false);
        return { text, reason, answer: resolve };
      });
    });
  const ringBell = (kind: "visual" | "sound") => {
    if (kind === "sound") playChime();
    if (!document.hasFocus()) void host.bounceDock().catch(() => undefined);
  };
  // Task 8.23: a drag on the sidebar's edge sets its width live; the drop saves it.
  const [sideDrag, setSideDrag] = useState<number>();
  const sideWidth = sideDrag ?? appSettings.sidebarWidth;
  const sideCollapsed = appSettings.sidebarCollapsed;
  const saveSidebar = async (patch: Pick<Partial<AppSettings>, "sidebarWidth" | "sidebarCollapsed">) => {
    try {
      const now = await host.settings();
      setAppSettings(await host.setSettings({ ...now, ...patch }));
    } catch (e) {
      setError(String(e));
    }
  };
  const sideResize = (e: React.PointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const el = e.currentTarget;
    const x0 = e.clientX;
    const w0 = sideWidth;
    const widthAt = (x: number) => Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, Math.round(w0 + x - x0)));
    el.setPointerCapture(e.pointerId);
    const move = (m: PointerEvent) => setSideDrag(widthAt(m.clientX));
    const up = (u: PointerEvent) => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
      const w = widthAt(u.clientX);
      void saveSidebar({ sidebarWidth: w }).finally(() => setSideDrag(undefined));
      refocus();
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
  };
  const [settingsSection, setSettingsSection] = useState<SettingsSection>("statuses");
  // Settings › Keys: the palette shows the chords the person set.
  const [keyBindings, setKeyBindings] = useState<KeyBindings>({});
  useEffect(() => {
    let gone = false;
    let off: (() => void) | undefined;
    const load = () => void host.keyBindings().then((b) => !gone && setKeyBindings(b), () => undefined);
    load();
    void host.onKeyBindings(load).then((o) => (gone ? o() : (off = o)));
    return () => {
      gone = true;
      off?.();
    };
  }, [host]);
  // The enabled plugins' contributions, composed in plugin order (add-plugin-architecture D7).
  const boundActions = useMemo(() => withBindings(ACTIONS, keyBindings), [keyBindings]);
  const plugins = usePlugins(host, boundActions);
  const decorate = useMemo<Decorate>(
    () => ({
      row: (side) => plugins.composed.rowItems({ id: side.id, tabIds: side.tabs.map((t) => t.id), shownTabId: pick(side.tabs.map((t) => t.id), tabBySide[side.id]) }),
      tab: (side, tabId) => {
        const tab = side.tabs.find((t) => t.id === tabId);
        return plugins.composed.tabItems({ id: tabId, sideTabId: side.id, paneIds: tab ? panesOf(tab.layout) : [] });
      },
    }),
    [plugins.composed, tabBySide],
  );
  // A plugin's questions, one dialog at a time: a second waits for the first's answer.
  const [asking, setAsking] = useState<{ n: number; question: Question; answer(a: Answer): void }>();
  const askQueue = useRef<Promise<unknown>>(Promise.resolve());
  const askCount = useRef(0);
  const shownAsk = useRef<{ n: number; answer(a: Answer): void }>(undefined);
  const ask = (question: Question): Promise<Answer> => {
    const next = askQueue.current.then(
      () =>
        new Promise<Answer>((resolve) => {
          const shown = { n: ++askCount.current, question, answer: resolve };
          shownAsk.current = shown;
          setAsking(shown);
        }),
    );
    askQueue.current = next.catch(() => undefined);
    return next;
  };
  // Answers only the question on screen: a second answer before the next render (Escape and a
  // click outside together) must not dismiss the next question unanswered.
  const answerAsk = (n: number, a: Answer) => {
    const cur = shownAsk.current;
    if (cur?.n !== n) return;
    shownAsk.current = undefined;
    setAsking(undefined);
    cur.answer(a);
    refocus();
  };
  /** Runs a plugin action; from a row's menu it runs on that row and its shown tab's shell. */
  const runPluginAction = (id: string, rowId?: string) => {
    const a = plugins.composed.actions.find((x) => x.id === id);
    if (!a) return;
    const row = rowId && rowId !== activeSide?.id ? sideTabs.find((sd) => sd.id === rowId) : undefined;
    const rowShell = row && (() => {
      const shown = row.tabs.find((t) => t.id === tabBySide[row.id]) ?? row.tabs[0];
      const terms = shown ? terminalsOf(shown.layout) : [];
      const last = shown && lastTerminal.current[shown.id];
      return last && terms.includes(last) ? last : terms[0];
    })();
    const cx = {
      windowId,
      bridge: plugins.bridge(a.plugin),
      ask,
      openUrl: (url: string) => {
        if (!/^https?:\/\//i.test(url)) return Promise.reject(new Error("only http and https links open"));
        return host.openUrl(url);
      },
      ...(row ? { sideTabId: row.id, ...(rowShell ? { shellPaneId: rowShell } : {}) } : {
        ...(activeSide ? { sideTabId: activeSide.id } : {}),
        ...(focused ? { paneId: focused } : {}),
        ...(shellPane ? { shellPaneId: shellPane } : {}),
      }),
    };
    void Promise.resolve()
      .then(() => a.run(cx))
      .catch((e: unknown) => setError(`${a.label}: ${String(e)}`));
  };
  // Plugins hear which row this window shows (D3).
  const selectedRow = activeSide?.id;
  useEffect(() => {
    if (selectedRow) void host.rowSelected(windowId, selectedRow).catch(() => undefined);
  }, [host, windowId, selectedRow]);
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
  // Which status the editor opens on: the side tab's, or its active tab's (a tab's chip).
  const [statusFor, setStatusFor] = useState<"side" | "tab">("side");
  // Task 8.8: one zoom for every window, saved with the settings; the host applies it.
  const zoom = async (dir: 1 | -1 | 0) => {
    try {
      const now = await host.settings();
      const zoomPercent = dir ? stepZoom(now.zoomPercent, dir) : 100;
      if (zoomPercent !== now.zoomPercent) await host.setSettings({ ...now, zoomPercent });
      // The level shows briefly; a later press keeps it up longer.
      const text = `Zoom ${zoomPercent}%`;
      setNotice(text);
      setTimeout(() => setNotice((n) => (n === text ? undefined : n)), 1500);
    } catch (e) {
      setError(String(e));
    }
  };
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
  // Find in all panes (task 8.46): every terminal in the window, labelled as its notification.
  const allTerminals = sideTabs.flatMap((sd) =>
    sd.tabs.flatMap((t) => {
      const list = terminalsOf(t.layout);
      return list.map((paneId, i) => ({ paneId, label: `${sideTabName(sd)} › ${tabName(t)}${list.length > 1 ? ` · pane ${i + 1}` : ""}` }));
    }),
  );
  const pickFound = (hit: FindHit) => {
    setFindingAll(false);
    revealPane(hit.paneId);
    setTimeout(() => paneEvent(hit.paneId, new CustomEvent(REVEAL_ROWS_EVENT, { detail: hit.rows })));
  };
  const revealRef = useRef(revealPane);
  revealRef.current = revealPane;
  const openRef = useRef(openDoc);
  openRef.current = openDoc;
  useEffect(() => {
    const offs: (() => void)[] = [];
    let gone = false;
    const keep = (o: () => void) => (gone ? o() : offs.push(o));
    void host.onRevealPane((p) => revealRef.current(p)).then(keep);
    void host.onChime(playChime).then(keep);
    // Files dropped on a terminal go in as its shell reads them (task 8.43); anywhere else,
    // the tab bar first of all, they open in text tabs.
    void host
      .onFileDrop((paths, at) => {
        const term = at && document.elementFromPoint(at.x, at.y)?.closest<HTMLElement>(".term-pane[data-pane]");
        if (!term) return paths.forEach((p) => openRef.current(p, false));
        paneEvent(term.dataset.pane!, new CustomEvent(INSERT_EVENT, { detail: { text: shellPaths(paths) } }));
      })
      .then(keep);
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
      // A side tab's only tab merges its side tab in, so its variables and pins come along.
      const from = sideTabs.find((s) => s.tabs.some((t) => t.id === tabId));
      if (from && from.id !== to.id && from.tabs.length === 1) return startMerge(from.id, to.id);
      followTab(tabId);
      return void send({ type: "MoveTab", tabId, toSideTabId: to.id, index: to.tabs.length });
    }
    followTab(tabId);
    void send({ type: "MoveTabToNewSideTab", tabId, windowId, index: target.newAt });
  };

  // Right-click menus on the sidebar (task 8.7). Rename edits the name in place.
  const [menu, setMenu] = useState<{ x: number; y: number; target: MenuTarget }>();
  // Terminal links (task 8.49): a click copies and says so at the pointer; a right click opens
  // the apps that fit the link's kind.
  const [linkTip, setLinkTip] = useState<{ text: string; x: number; y: number; key: number }>();
  const [linkMenu, setLinkMenu] = useState<{ link: TerminalLink; sideId: string; paneId: string; x: number; y: number }>();
  // Every other right click opens the pixel menu with the items macOS would show there (task
  // 8.55): a terminal's, an editor's or a field's. The Edit items go down the responder chain,
  // as the Edit menu's do, after the element right-clicked takes the focus back.
  const [editMenu, setEditMenu] = useState<{ kind: "term" | "text" | "field" | "selection"; el: HTMLElement; x: number; y: number; selected?: string }>();
  useEffect(() => {
    const onMenu = (e: MouseEvent) => {
      if (e.defaultPrevented) return;
      e.preventDefault();
      const t = e.target instanceof Element ? e.target : null;
      const field = t?.closest<HTMLElement>("input:not([type=checkbox]):not([type=radio]):not([type=range]), textarea:not(.xterm-helper-textarea), [contenteditable=true]:not(.cm-content)");
      // A text pane sits in a pane element too, so the editor is looked for first.
      const text = t?.closest<HTMLElement>(".text-pane")?.querySelector<HTMLElement>(".cm-content");
      const term = t?.closest<HTMLElement>(".term-pane[data-pane]")?.querySelector<HTMLElement>("textarea.xterm-helper-textarea");
      const el = field ?? text ?? term;
      // Selected text elsewhere, as in a transcript pane: only Copy, as macOS offers.
      const selected = window.getSelection()?.toString();
      if (!el && selected && t instanceof HTMLElement) return setEditMenu({ kind: "selection", el: t, x: e.clientX, y: e.clientY, selected });
      if (!el) return;
      el.focus();
      setEditMenu({ kind: field ? "field" : text ? "text" : "term", el, x: e.clientX, y: e.clientY });
    };
    document.addEventListener("contextmenu", onMenu);
    return () => document.removeEventListener("contextmenu", onMenu);
  }, []);
  useEffect(() => {
    if (!linkTip) return;
    const t = setTimeout(() => setLinkTip(undefined), 1600);
    return () => clearTimeout(t);
  }, [linkTip]);
  const copyLink = (link: TerminalLink, at: { x: number; y: number }) => {
    const what = link.kind === "url" ? "link" : link.kind === "path" ? "path" : "address";
    void (navigator.clipboard?.writeText(link.text) ?? Promise.reject(new Error("no clipboard"))).then(
      () => setLinkTip({ text: `Copied ${what}`, x: at.x, y: at.y, key: Date.now() }),
      () => setError(`Could not copy the ${what}.`),
    );
  };
  const runLink = (id: string, m: NonNullable<typeof linkMenu>) => {
    const fail = (e: unknown) => setError(String(e));
    const { link, sideId, paneId } = m;
    if (id === "copy") return copyLink(link, m);
    if (id === "browser") return void host.openUrl(link.text).catch(fail);
    if (id === "mail") return void host.openUrl(`mailto:${link.text}`).catch(fail);
    if (id === "text") return void host.openPath(sideId, paneId, link.text, null).then((d) => revealDoc(d.id), fail);
    if (id === "reveal" || id === "default") return void host.openPathWith(paneId, link.text, id === "reveal").catch(fail);
  };
  // Where a name is being edited: the sidebar, or the tab bar, which shows the same tabs.
  const [renaming, setRenaming] = useState<{ id: string; in: "list" | "bar" }>();
  const [closingSide, setClosingSide] = useState<SideTab>();
  // The side tab or tab whose icon is being picked, and where the picker opens (task 8.16).
  const [picking, setPicking] = useState<{ id: string; x: number; y: number }>();
  // A click on a sidebar status chip: the saved statuses for a quick pick, the editor behind them.
  const [quickStatus, setQuickStatus] = useState<{ sideTabId: string; tabId?: string; x: number; y: number }>();
  const rename = (id: string, title: string | null) => {
    setRenaming(undefined);
    const side = sideTabs.some((sd) => sd.id === id);
    void send(side ? { type: "RenameSideTab", sideTabId: id, title } : { type: "RenameTab", tabId: id, title });
    refocus();
  };
  const closeSide = (side: SideTab) => {
    setClosingSide(undefined);
    void Promise.resolve(endToast(false)).then(() => closeWith(
      side.tabs.flatMap((t) => panesOf(t.layout)),
      { type: "CloseSideTab", sideTabId: side.id },
    ));
  };
  const editStatus = (side: string, tab?: string) => {
    if (tab) selectTab(side, tab);
    else setSideTabId(side);
    setStatusFor(tab ? "tab" : "side");
    setSheet("status");
  };
  const editItems = (m: NonNullable<typeof editMenu>): MenuEntry[] => {
    const bound = withBindings(ACTIONS, keyBindings);
    const hint = (id: string) => {
      const accel = bound.find((a) => a.id === id)?.accel;
      return accel ? chordLabel(accel) : undefined;
    };
    // The menu closes first; the element takes the focus back before the action runs.
    const back = (run: () => void) => () =>
      setTimeout(() => {
        m.el.focus();
        run();
      });
    const edit = (action: EditAction, label: string, chord: string): MenuEntry => ({
      id: action,
      label,
      hint: chord,
      run: back(() => void host.editCommand(action).catch((e: unknown) => setError(String(e)))),
    });
    const action = (id: string, label: string, danger?: boolean): MenuEntry => ({ id, label, hint: hint(id), danger, run: back(() => act(id)) });
    if (m.kind === "selection") {
      const copy = () => void (navigator.clipboard?.writeText(m.selected ?? "") ?? Promise.reject(new Error("no clipboard"))).catch(() => setError("Could not copy the selection."));
      return [{ id: "copy", label: "Copy", hint: "⌘C", run: copy }];
    }
    const clip = [edit("copy", "Copy", "⌘C"), edit("paste", "Paste", "⌘V")];
    if (m.kind === "field") return [edit("cut", "Cut", "⌘X"), ...clip, action("select_all", "Select All")];
    if (m.kind === "text") {
      return [edit("undo", "Undo", "⌘Z"), edit("redo", "Redo", "⇧⌘Z"), "-", edit("cut", "Cut", "⌘X"), ...clip, action("select_all", "Select All"), "-", action("find", "Find…")];
    }
    return [
      ...clip,
      action("select_all", "Select All"),
      "-",
      action("find", "Find…"),
      action("clear_to_start", "Clear to Start"),
      "-",
      action("split_right", "Split Right"),
      action("split_down", "Split Down"),
      action("maximize_pane", "Maximize Pane"),
      action("pane_to_tab", "Move Pane to New Tab"),
      "-",
      action("close", "Close Pane", true),
    ];
  };

  const menuItems = (target: MenuTarget): MenuEntry[] => {
    if (target.kind === "list") {
      return [
        { id: "new-side", label: "New side tab", hint: "⇧⌘N", run: () => act("new_side_tab") },
        { id: "new-window", label: "New window", hint: "⌘N", run: () => act("new_window") },
        "-",
        {
          id: "sort",
          label: "Sort side tabs",
          items: SORTS.map((m) => ({ id: `sort:${m}`, label: m === sort ? `${m} ✓` : m, run: () => changeSort(m) })),
        },
      ];
    }
    const side = sideTabs.find((sd) => sd.id === target.sideTabId);
    if (!side) return [];
    const others = sideTabs.filter((sd) => sd.id !== side.id);
    if (target.kind === "side") {
      return [
        { id: "rename", label: "Rename…", run: () => setRenaming({ id: side.id, in: "list" }) },
        { id: "icon", label: "Change icon…", run: () => menu && setPicking({ id: side.id, x: menu.x, y: menu.y }) },
        // Adds into this row, so on a one-tab row it makes a folder (task 8.81).
        {
          id: "new-tab",
          label: side.tabs.length > 1 ? "Add tab to this folder" : "Add tab to this row",
          run: () => {
            followNewTab(side.id);
            void send({ type: "CreateTab", sideTabId: side.id });
          },
        },
        { id: "status", label: "Edit status…", run: () => editStatus(side.id) },
        // Plugin actions that work on a row (add-claude-insights 3.1).
        ...((): MenuEntry[] => {
          const own = plugins.composed.actions.filter((a) => a.rowMenu);
          return own.length ? ["-", ...own.map((a) => ({ id: `plugin:${a.id}`, label: a.label, ...(a.accel ? { hint: chordLabel(a.accel) } : {}), run: () => runPluginAction(a.id, side.id) }))] : [];
        })(),
        "-",
        {
          id: "merge",
          label: "Merge into",
          disabled: !others.length,
          items: others.map((o) => ({ id: `merge:${o.id}`, label: sideTabName(o, docs), run: () => startMerge(side.id, o.id) })),
        },
        { id: "window", label: "Move to new window", run: () => void send({ type: "MoveSideTabToNewWindow", sideTabId: side.id }) },
        "-",
        { id: "close", label: "Close side tab…", danger: true, run: () => setClosingSide(side) },
      ];
    }
    const tab = side.tabs.find((t) => t.id === target.tabId);
    if (!tab) return [];
    return [
      { id: "rename", label: "Rename…", run: () => setRenaming({ id: tab.id, in: "list" }) },
      { id: "icon", label: "Change icon…", run: () => menu && setPicking({ id: tab.id, x: menu.x, y: menu.y }) },
      { id: "status", label: "Edit status…", run: () => editStatus(side.id, tab.id) },
      "-",
      {
        id: "move",
        label: "Move to",
        items: [
          { id: "move:new", label: "New side tab", disabled: side.tabs.length === 1, run: () => moveTab(tab.id, { newAt: sideTabs.length }) },
          { id: "move:window", label: "New window", run: () => void send({ type: "MoveTabToNewWindow", tabId: tab.id }) },
          ...(others.length ? (["-"] as MenuEntry[]) : []),
          ...others.map((o) => ({ id: `move:${o.id}`, label: sideTabName(o, docs), run: () => moveTab(tab.id, { side: o.id }) })),
        ],
      },
      "-",
      { id: "close", label: "Close tab", danger: true, run: () => closeTab(tab.id) },
    ];
  };

  // Drop a side tab onto another (D5): straight in without a clash, else the dialog asks.
  // A side tab of any window: a tab dragged from another one may bring its side tab along.
  const anySide = (id: string) => sideTabs.find((s) => s.id === id) ?? ws.windows.flatMap((w) => w.sideTabs).find((s) => s.id === id);
  /**
   * Merges side tab `fromId` into `intoId` (D5). `after` runs once the merge is done, as the
   * dock or the move into place that follows when a side tab's only tab is dropped (task 8.70).
   */
  const startMerge = (fromId: string, intoId: string, after?: AfterMerge) => {
    const from = anySide(fromId);
    const into = anySide(intoId);
    if (!from || !into || from.id === into.id) return;
    // A removal still on its toast is done first: the merge moves what it would delete.
    void Promise.resolve(endToast(false)).then(() => host.mergePreview(from.id, into.id, sideTabName(from, docs)).then(
      (clashes) => {
        if (clashes.length) setMerging({ from, into, clashes, after });
        else
          finishMerge(from, into, { keepBoth: false }, after).catch((e: unknown) => {
            report(e);
            refocus();
          });
      },
      (e: unknown) => {
        report(e);
        refocus();
      },
    ));
  };
  // Rejects when the merge fails, so an open dialog keeps itself and shows why.
  const finishMerge = (from: SideTab, into: SideTab, resolution: MergeResolution, after?: AfterMerge) => {
    const fromName = sideTabName(from, docs);
    const intoName = sideTabName(into, docs);
    return host.mergeSideTabs(from.id, into.id, fromName, resolution).then((done) => {
      setMerging(undefined);
      const renamed = done.renames.map(([a, b]) => `${a} → ${b}`).join(", ");
      const text = `Merged ${fromName} into ${intoName}${renamed ? `; kept ${renamed}` : ""}.`;
      showToast({ text, undo: () => host.undoMerge(done.undoId).then(refocus, report) });
      if (activeSide?.id === from.id) setSideTabId(into.id);
      if (after) after.run();
      else refocus();
    });
  };
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => toastRef.current === toast && endToast(false), MERGE_UNDO_MS);
    return () => clearTimeout(t);
  }, [toast]);
  // A removed variable, pin or command leaves the sidebar at once and is deleted when its
  // toast goes. Undo works for a secret too: the panel never holds a secret's value (D35).
  const removeLater = (key: Removal, text: string, remove: () => Promise<unknown>) => {
    const forget = () => setRemoving((r) => r.filter((x) => x !== key));
    setRemoving((r) => [...r, key]);
    showToast({
      text,
      removes: key,
      undo: forget,
      done: () => {
        forget();
        return remove().catch(report);
      },
    });
    return Promise.resolve();
  };
  const removeVariable = (side: string, name: string) =>
    removeLater({ kind: "variable", side, name }, `Removed ${name}.`, () => host.removeVariable(side, name));
  const shownCommands = useMemo(
    () => (removing.length ? commands.filter((c) => !removing.some((r) => r.kind === "command" && r.id === c.id)) : commands),
    [commands, removing],
  );
  // Adding a name whose removal waits on the toast deletes the old one first, so the late
  // delete cannot take the new variable.
  const addVariable = async (side: string, v: Parameters<typeof host.addVariable>[1]) => {
    const r = toastRef.current?.removes;
    if (r?.kind === "variable" && r.side === side && r.name === v.name) {
      toastRef.current = undefined;
      setToast(undefined);
      setRemoving((x) => x.filter((k) => k !== r));
      await host.removeVariable(side, v.name);
    }
    return host.addVariable(side, v);
  };

  // A tab dragged from another window (task 8.1): the core says where the cursor is; this
  // window lights the target under it and, on a drop, moves the tab there.
  const [remote, setRemote] = useState<DragPoint | null>(null);
  const remoteRef = useRef<(p: DragPoint) => void>(() => {});
  remoteRef.current = (p: DragPoint) => {
    setRemote(null);
    if (p.sideTabId !== undefined) return remoteRow(p.sideTabId, p.x, p.y);
    // A side tab's only tab merges its side tab in first, so its variables and pins come
    // along (task 8.70); a gap moves the whole side tab in the core.
    const from = ws.windows.flatMap((w) => w.sideTabs).find((s) => s.tabs.some((t) => t.id === p.tabId));
    const only = from?.tabs.length === 1;
    // On a pane's edge it shows beside that tab (task 8.75).
    const docked = dockAt(p.tabId, p.x, p.y);
    if (docked && docked.side !== "center" && !docked.small) return placeBeside(p.tabId, docked.area, docked.side);
    const target = remoteTarget(p.x, p.y);
    // Anywhere else in the panes, a window's only row joins this window as a row and shows.
    if (only && from && !target) {
      setFollow((w) => sidesOf(w).some((s) => s.id === from.id) && (setSideTabId(from.id), refocus(), true));
      return void send({ type: "MoveSideTab", sideTabId: from.id, windowId, index: sideTabs.length });
    }
    if (only && from && target?.kind !== "end" && target?.kind !== "gap") {
      if (target?.kind === "side") return startMerge(from.id, target.id);
      // Dropped on a tab listed in a folder, it takes that tab's place there (task 8.76).
      if (target?.kind === "kid") {
        const index = anySide(target.folder)?.tabs.findIndex((t) => t.id === target.id) ?? -1;
        const run = () => {
          followTab(p.tabId);
          send({ type: "MoveTab", tabId: p.tabId, toSideTabId: target.folder, index }).then(refocus, report);
        };
        return startMerge(from.id, target.folder, { run });
      }
      // Dropped on a tab of the tab bar, it takes that tab's place once merged in.
      const into = activeSide;
      const index = target?.kind === "tab" ? tabs.findIndex((t) => t.id === target.id) : -1;
      const place = () => {
        followTab(p.tabId);
        send({ type: "MoveTab", tabId: p.tabId, toSideTabId: into!.id, index }).then(refocus, report);
      };
      if (into) return startMerge(from.id, into.id, index >= 0 ? { run: place } : undefined);
    }
    const cmd: Command | undefined =
      target?.kind === "end"
        ? { type: "MoveTabToNewSideTab", tabId: p.tabId, windowId, index: sideTabs.length }
        : target?.kind === "gap"
          ? { type: "MoveTabToNewSideTab", tabId: p.tabId, windowId, index: target.index }
          : target?.kind === "side"
            ? { type: "MoveTab", tabId: p.tabId, toSideTabId: target.id, index: sideTabs.find((s) => s.id === target.id)?.tabs.length ?? 0 }
            : target?.kind === "kid"
              ? { type: "MoveTab", tabId: p.tabId, toSideTabId: target.folder, index: anySide(target.folder)?.tabs.findIndex((t) => t.id === target.id) ?? 0 }
            : activeSide
              ? {
                  type: "MoveTab",
                  tabId: p.tabId,
                  toSideTabId: activeSide.id,
                  index: target?.kind === "tab" && tabs.some((t) => t.id === target.id) ? tabs.findIndex((t) => t.id === target.id) : tabs.length,
                }
              : { type: "MoveTabToNewSideTab", tabId: p.tabId, windowId, index: 0 };
    followTab(p.tabId);
    send(cmd).then(refocus, report);
  };
  /**
   * A folder row from another window, dropped here (task 8.86): on a row or a tab under a
   * folder it merges into that row, on a pane's edge it shows beside that tab, and anywhere
   * else it joins this window as a row, at a gap or at the end.
   */
  const remoteRow = (id: string, x: number, y: number) => {
    const docked = dockAt(`side:${id}`, x, y);
    if (docked && docked.side !== "center" && !docked.small) return placeBeside(`side:${id}`, docked.area, docked.side);
    const target = remoteTarget(x, y);
    if (target?.kind === "side") return startMerge(id, target.id);
    if (target?.kind === "kid") return startMerge(id, target.folder);
    const index = target?.kind === "gap" ? target.index : sideTabs.length;
    setFollow((w) => sidesOf(w).some((s) => s.id === id) && (setSideTabId(id), refocus(), true));
    void send({ type: "MoveSideTab", sideTabId: id, windowId, index });
  };
  useEffect(() => {
    const offs: (() => void)[] = [];
    let gone = false;
    const keep = (o: () => void) => (gone ? o() : offs.push(o));
    void host.onDragHover(setRemote).then(keep);
    void host.onDragDrop((p) => remoteRef.current(p)).then(keep);
    return () => {
      gone = true;
      offs.forEach((o) => o());
    };
  }, [host]);
  // The lit target follows the point; a class, not state: it changes 60 times a second.
  useEffect(() => {
    if (!remote) return;
    const el = remoteTarget(remote.x, remote.y)?.el;
    el?.classList.add("remote-hot");
    return () => el?.classList.remove("remote-hot");
  }, [remote]);

  // Task 8.32: a tab dragged over the shown panes docks on a side of the one under the pointer;
  // a pane header dragged to the tab bar becomes a tab. Both follow the pointer while a drag
  // lasts and change state only when the target changes.
  const [dock, setDock] = useState<Dock>();
  const dockRef = useRef(dock);
  dockRef.current = dock;
  const [tabGap, setTabGap] = useState<TabGap>();
  const tabGapRef = useRef(tabGap);
  tabGapRef.current = tabGap;
  /**
   * What a drag carries: a row (`side:<id>`), with all its tabs, or a tab, from this window or
   * another; `lone` when that is the row's every tab.
   */
  const dragSource = (dragId: string) => {
    if (dragId.startsWith("side:")) {
      const side = anySide(dragId.slice(5));
      return side && { side, tabs: side.tabs.map((t) => t.id), lone: true };
    }
    const side = ws.windows.flatMap((w) => w.sideTabs).find((s) => s.tabs.some((t) => t.id === dragId));
    return side && { side, tabs: [dragId], lone: side.tabs.length === 1 };
  };
  const dockAt = (dragId: string, x: number, y: number): Dock | undefined => {
    const el = document
      .elementsFromPoint(x, y)
      .map((e) => e.closest<HTMLElement>(".app-pane .ao-tp[data-pane]"))
      .find(Boolean);
    const pane = el?.dataset.pane;
    const area = pane ? areaOfPane(pane) : undefined;
    const src = dragSource(dragId);
    if (!pane || !area || !src) return undefined;
    // A tab cannot go beside itself, nor a folder beside one of its tabs.
    if (src.tabs.includes(area.tab.id)) return undefined;
    // The drop splits the tab's whole area, so the area is the box.
    const rects = panesOf(area.tab.layout).flatMap((p) => {
      const at = document.querySelector<HTMLElement>(`.app-pane .ao-tp[data-pane="${p}"]`);
      return at ? [at.getBoundingClientRect()] : [];
    });
    const left = Math.min(...rects.map((r) => r.left));
    const top = Math.min(...rects.map((r) => r.top));
    const box = { left, top, width: Math.max(...rects.map((r) => r.right)) - left, height: Math.max(...rects.map((r) => r.bottom)) - top };
    const side = dockZone(box, x, y);
    if (!side) return undefined;
    if (side === "center") return { pane, area: area.tab.id, side, box, small: false };
    return { pane, area: area.tab.id, side, box, small: !showBeside(shownView, area.tab.id, src.tabs, side, "preview") };
  };
  const dockAtRef = useRef(dockAt);
  dockAtRef.current = dockAt;
  useEffect(() => {
    if (!dragging) return;
    const pane = dragging.startsWith("pane:");
    const move = (e: PointerEvent) => {
      if (pane) {
        const g = tabGapAt(e.clientX, e.clientY);
        setTabGap((p) => (JSON.stringify(p) === JSON.stringify(g) ? p : g));
      } else {
        const d = dockAtRef.current(tabOfDrag(dragging), e.clientX, e.clientY);
        setDock((p) => (sameDock(p, d) ? p : d));
      }
    };
    window.addEventListener("pointermove", move);
    return () => {
      window.removeEventListener("pointermove", move);
      setDock(undefined);
      setTabGap(undefined);
    };
  }, [dragging]);
  // A tab from another window docks the same way; the core sends where its cursor is.
  useEffect(() => {
    if (!remote) return;
    const d = dockAtRef.current(remote.sideTabId !== undefined ? `side:${remote.sideTabId}` : remote.tabId, remote.x, remote.y);
    setDock((p) => (sameDock(p, d) ? p : d));
    return () => setDock(undefined);
  }, [remote]);
  /** Docks a tab beside a pane of the active tab and focuses the first pane it brought. */
  const dockTab = (tabId: string, paneId: string, side: DockSide) => {
    const moving = ws.windows.flatMap((w) => w.sideTabs.flatMap((s) => s.tabs)).find((t) => t.id === tabId);
    const first = moving && panesOf(moving.layout)[0];
    const into = activeTab?.id;
    void send({ type: "DockTab", tabId, paneId, side }).then((ok) => {
      if (ok && first && into) focusIn(into, first);
      refocus();
    });
  };
  /** A folder's plain variables go with its tab from another window into a new row; secrets and pins stay. */
  const copyVariables = (from: string, to: string) =>
    void host
      .variables(from)
      .then((vars) =>
        Promise.all(
          vars.flatMap((v) =>
            v.value === null
              ? []
              : [
                  host.addVariable(to, {
                    name: v.name,
                    value: v.value,
                    mode: v.mode,
                    ttlSecs: v.expiresAt ? Math.max(1, Math.round((v.expiresAt - Date.now()) / 1000)) : null,
                    source: v.source ?? "typed",
                    originTabId: null,
                  }),
                ],
          ),
        ),
      )
      .catch(report);
  /**
   * Task 8.75: a tab, or a row with all its tabs, shows beside the shown tab `target`; a tab of
   * this window stays in its folder. One from another window moves here first: a window's only
   * row as it is, a folder's tab as a new row with the folder's plain variables.
   */
  const placeBeside = (dragId: string, target: string, side: DockSide) => {
    const src = dragSource(dragId);
    if (!src) return refocus();
    const show = (row: string) => {
      changeView((v) => showBeside(v ?? shownView, target, src.tabs, side, viewSplitId()) ?? v);
      selectTab(row, src.tabs[0]!);
      refocus();
    };
    if (sideTabs.some((s) => s.id === src.side.id)) return show(src.side.id);
    if (src.lone) {
      setFollow((w) => sidesOf(w).some((s) => s.id === src.side.id) && (show(src.side.id), true));
      return void send({ type: "MoveSideTab", sideTabId: src.side.id, windowId, index: sideTabs.length });
    }
    const tabId = src.tabs[0]!;
    setFollow((w) => {
      const row = sidesOf(w).find((s) => s.id !== src.side.id && s.tabs.some((t) => t.id === tabId));
      if (!row) return false;
      show(row.id);
      copyVariables(src.side.id, row.id);
      return true;
    });
    void send({ type: "MoveTabToNewSideTab", tabId, windowId, index: sideTabs.length });
  };
  /** A pane of the active tab becomes a tab at `index`, and that tab opens. */
  const paneToTab = (paneId: string, index: number) => {
    setFollow((w) => {
      for (const side of sidesOf(w))
        for (const t of side.tabs)
          if (t.layout.type === "pane" && t.layout.id === paneId) {
            selectTab(side.id, t.id);
            focusIn(t.id, paneId);
            refocus();
            return true;
          }
      return false;
    });
    void send({ type: "PaneToTab", paneId, index });
  };

  // The pending `dragBegin` of the tab being dragged; resolves to its token (task 8.1).
  const dragBegun = useRef<Promise<number | undefined> | undefined>(undefined);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }));
  const onDragEnd = ({ active, over }: DragEndEvent) => {
    // Read before the drag's state goes.
    const docked = dockRef.current;
    const gap = tabGapRef.current;
    setDragging(undefined);
    const tabId = tabOfDrag(String(active.id));
    const at = over && String(over.id);
    if (tabId.startsWith("pane:")) {
      if (gap) paneToTab(tabId.slice(5), gap.index);
      else refocus();
      return;
    }
    const beside = docked && docked.side !== "center" && !docked.small ? docked : undefined;
    if (tabId.startsWith("side:")) {
      // A row's drag is also the core's (tasks 8.83, 8.86): released over another window it
      // drops there, and this window, emptied, closes. A drop in this window cancels it.
      const begun = dragBegun.current;
      dragBegun.current = undefined;
      const local = !!beside || !!at?.startsWith("into:");
      void begun
        ?.then((token) => (token === undefined ? undefined : host.dragEnd(token, !local)))
        .then((end) => end && end.kind !== "local" && end.kind !== "cancelled" && refocus())
        .catch(report);
    }
    // A merge may open its dialog, which takes the focus; it gives it back when it closes.
    if (tabId.startsWith("side:") && beside) return placeBeside(tabId, beside.area, beside.side as DockSide);
    if (tabId.startsWith("side:") && at?.startsWith("into:")) {
      // The dropped row's tabs go last in the folder, open, with its first tab selected (task 8.80).
      const into = at.slice(5);
      const first = sideTabs.find((s) => s.id === tabId.slice(5))?.tabs[0]?.id;
      const run = () => {
        setOpenFolder({ id: into });
        // The merge's workspace update may already be in; else wait for it.
        const pick = (w: Workspace) => {
          const ok = !!first && sidesOf(w).some((s) => s.id === into && s.tabs.some((t) => t.id === first));
          if (ok) selectTab(into, first!);
          return ok;
        };
        if (!pick(wsNow.current)) setFollow(pick);
        refocus();
      };
      return startMerge(tabId.slice(5), into, { run });
    }
    if (tabId.startsWith("side:")) return refocus();
    // The core saw where the cursor was (task 8.1): only a release over this window drops here.
    // A fast flick ends before `dragBegin` answers, so the end waits for its token. A drag the
    // core no longer tracks (it timed out, or never began) is left to this window.
    const begun = dragBegun.current;
    dragBegun.current = undefined;
    const ended = (begun ?? Promise.resolve(undefined)).then(
      (token): Promise<DragEnd> => (token === undefined ? Promise.resolve({ kind: "cancelled" }) : host.dragEnd(token, true)),
    );
    ended.then(
      (end) => {
        if (end.kind !== "local" && end.kind !== "cancelled") refocus();
        // The middle of a pane, or a pane too small to halve, cancels.
        else if (beside) placeBeside(tabId, beside.area, beside.side as DockSide);
        else if (docked) refocus();
        else dropHere(tabId, at);
      },
      (e: unknown) => {
        report(e);
        refocus();
      },
    );
  };
  const dropHere = (tabId: string, at: string | null) => {
    refocus();
    const from = sideTabs.find((s) => s.tabs.some((t) => t.id === tabId));
    if (!at || !from) return;
    // A side tab's only tab takes its side tab along: to a gap the side tab moves, and into
    // another side tab it merges, so its variables and pins stay (task 8.70).
    const only = from.tabs.length === 1;
    // The only tab of a side tab dropped next to it would make the same sidebar again.
    const idx = sideTabs.findIndex((s) => s.id === from.id);
    const gap = at === DROP_END ? sideTabs.length : at.startsWith("gap:") ? Number(at.slice(4)) : -1;
    if (only && (gap === idx || gap === idx + 1)) return;
    if (at.startsWith("into:")) {
      const side = at.slice(5);
      if (side !== from.id) moveTab(tabId, { side });
    } else if (gap >= 0) {
      moveTab(tabId, { newAt: gap });
    } else {
      // A tab in the tab bar, or one listed under a folder: the dropped tab takes its place.
      const target = tabOfDrag(at);
      const to = sideTabs.find((s) => s.tabs.some((t) => t.id === target));
      if (!to || target === tabId) return;
      const index = to.tabs.findIndex((t) => t.id === target);
      const place = () => void send({ type: "MoveTab", tabId, toSideTabId: to.id, index }).then(refocus, report);
      if (to.id !== from.id && only) return startMerge(from.id, to.id, { run: place });
      if (to.id !== from.id) followTab(tabId);
      void send({ type: "MoveTab", tabId, toSideTabId: to.id, index });
    }
  };

  const paletteItems: PaletteItem[] = [
    ...paletteActions(withBindings(ACTIONS, keyBindings))
      .filter((a) => a.id !== "merge_windows" || ws.windows.length > 1)
      .map((a) => ({ id: a.id, label: a.label, group: "Actions", hint: chordLabel(a.accel), run: () => act(a.id) })),
    ...plugins.composed.actions.map((a) => ({
      id: a.id,
      label: a.label,
      group: "Plugins",
      ...(a.accel ? { hint: chordLabel(a.accel) } : {}),
      run: () => runPluginAction(a.id),
    })),
    ...(activeTab
      ? [
          { id: "move:new", label: "New side tab", group: "Move tab to…", run: () => moveTab(activeTab.id, { newAt: sideTabs.length }) },
          { id: "move:window", label: "New window", group: "Move tab to…", run: () => void send({ type: "MoveTabToNewWindow", tabId: activeTab.id }) },
          ...sideTabs
            .filter((s) => s.id !== activeSide?.id)
            .map((s) => ({ id: `move:${s.id}`, label: sideTabName(s), group: "Move tab to…", run: () => moveTab(activeTab.id, { side: s.id }) })),
        ]
      : []),
    // Task 8.32: the keyboard way to dock a tab beside the focused pane, and to undo it.
    ...(activeTab && focused && panesOf(activeTab.layout).includes(focused)
      ? (["right", "down"] as const).flatMap((side) =>
          tabs
            .filter((t) => t.id !== activeTab.id)
            .map((t) => ({
              id: `dock:${side}:${t.id}`,
              label: tabName(t, docs),
              group: side === "right" ? "Split right with tab…" : "Split down with tab…",
              run: () => dockTab(t.id, focused, side),
            })),
        )
      : []),
    // Tasks 8.71, 8.75: back to one tab, or one tab fewer on screen.
    ...(activeTab && openTabs.length > 1
      ? [
          { id: "view:alone", label: "Show only this tab", group: "Actions", run: () => changeView(() => ({ type: "pane", id: activeTab.id })) },
          ...areas
            .filter((x) => x.tab.id !== activeTab.id)
            .map((x) => ({ id: `view:hide:${x.tab.id}`, label: areaName(x), group: "Hide from the window…", run: () => changeView((v) => hideRow(v, x.tab.id)) })),
        ]
      : []),
    ...SORTS.map((m) => ({ id: `sort:${m}`, label: m === sort ? `${m} (now)` : m, group: "Sort side tabs", run: () => changeSort(m) })),
    ...sideTabs.map((s) => ({ id: `go:${s.id}`, label: sideTabName(s), group: "Go to side tab", run: () => setSideTabId(s.id) })),
    ...(activeSide
      ? sideTabs
          .filter((s) => s.id !== activeSide.id)
          .map((s) => ({ id: `merge:${s.id}`, label: sideTabName(s), group: "Merge side tab into…", run: () => startMerge(activeSide.id, s.id) }))
      : []),
  ];

  const dragTab = dragging ? allTabs.find((t) => t.id === tabOfDrag(dragging)) : undefined;
  const dragPane = dragging?.startsWith("pane:") ? dragging.slice(5) : undefined;
  const dragSideTab = dragging?.startsWith("side:") ? sideTabs.find((s) => s.id === dragging.slice(5)) : undefined;
  // Every shown row's panes in one tree; a maximized pane shows alone (task 8.39).
  const screen = maxPane && activeTab ? leaf(activeTab.layout, maxPane) : shownView && composeView(shownView, (id) => areas.find((x) => x.tab.id === id)?.tab.layout);
  /** A folder's tabs for a pixel tip: each with its directory, the shown one marked (task 8.74). */
  const tabLines = (side: SideTab, shown: string | undefined) =>
    folderLines(side.tabs, shown, (id) => tabName(side.tabs.find((t) => t.id === id)!, docs), subtitles);
  const multi = panes.length > 1 || (screen ? panesOf(screen).length : 0) > 1;

  /** A split pane's header text: the document, the transcript or the shell's live subtitle. */
  const paneHeadName = (paneId: string, transcript: boolean) => {
    if (transcript) return "transcript";
    const own = areaOfPane(paneId)?.tab;
    const docId = own ? docOf(own.layout, paneId) : undefined;
    if (docId) return docs[docId]?.title ?? "text";
    return paneSubtitle(labels[paneId], !!agents[paneId]) ?? "shell";
  };
  return (
    <>
      <DndContext
        sensors={sensors}
        collisionDetection={collide}
        // The app places the focus after a drop: the terminal, or a merge dialog's field.
        accessibility={{ restoreFocus: false }}
        onDragStart={({ active }) => {
          const id = String(active.id);
          setDragging(id);
          // A tab of the tab bar, or one listed under a folder in the sidebar (task 8.70). A
          // one-tab row carries its tab, which takes the row along (task 8.83); a folder row
          // carries itself (task 8.86). Either can go to another window.
          const row = id.startsWith("side:") ? sideTabs.find((s) => s.id === id.slice(5)) : undefined;
          const tab = row ? (row.tabs.length === 1 ? row.tabs[0] : undefined) : allTabs.find((t) => t.id === tabOfDrag(id));
          const item: [DragItem, string] | undefined = tab
            ? [{ tabId: tab.id }, tabName(tab, docs)]
            : row
              ? [{ sideTabId: row.id }, sideTabName(row, docs)]
              : undefined;
          dragBegun.current = item
            ? host.dragBegin(...item).catch((e: unknown) => {
                report(e);
                return undefined;
              })
            : undefined;
          // Esc cancels the drag; the keyboard must not be in a terminal for that.
          (document.activeElement as HTMLElement | null)?.blur();
        }}
        onDragEnd={onDragEnd}
        onDragCancel={({ active }) => {
          setDragging(undefined);
          const begun = dragBegun.current;
          dragBegun.current = undefined;
          void begun?.then((token) => {
            if (token !== undefined) host.dragEnd(token, false).catch(report);
          });
          refocus();
        }}
      >
        <div
          className="app"
          data-context={mode}
          data-side={sideCollapsed ? "collapsed" : undefined}
          data-resizing={sideDrag === undefined ? undefined : ""}
          style={{ "--side-w": sideCollapsed ? "var(--side-icons)" : `${sideWidth}px` } as React.CSSProperties}
        >
          <aside className="app-side">
            {/* Task 8.23: hovering the sidebar shows this; collapsed, only the rows' icons stay. */}
            <button
              type="button"
              className="side-toggle"
              aria-label={sideCollapsed ? "Expand sidebar" : "Collapse sidebar"}
              aria-expanded={!sideCollapsed}
              title={sideCollapsed ? "Expand sidebar" : "Collapse sidebar"}
              onPointerDown={(e) => e.stopPropagation()}
              onClick={() => {
                void saveSidebar({ sidebarCollapsed: !sideCollapsed });
                refocus();
              }}
            >
              <i className={`ao-i ${sideCollapsed ? "ao-i-chevr" : "ao-i-chevr flip"}`} />
            </button>
            {!sideCollapsed && (
              <div
                className="side-resize"
                role="separator"
                aria-orientation="vertical"
                aria-label="Sidebar width"
                aria-valuenow={sideWidth}
                aria-valuemin={SIDEBAR_MIN}
                aria-valuemax={SIDEBAR_MAX}
                title="Drag to resize, double-click for the default width"
                onPointerDown={sideResize}
                onDoubleClick={() => void saveSidebar({ sidebarWidth: SIDEBAR_WIDTH })}
              />
            )}
            {/* The title bar is an overlay (D20): this strip holds the traffic lights and moves the window. */}
            <div className="app-traffic" data-tauri-drag-region>
              <span className="app-wordmark" data-tauri-drag-region aria-label="Terminescu">
                TERMIN<b>ESCU</b>
              </span>
            </div>
            <SideTabList
              decorate={decorate}
              sideTabs={shownSides}
              statuses={statuses}
              builtins={statusConfig.builtins}
              docs={docs}
              subtitles={subtitles}
              activity={activity}
              sort={sort}
              activeId={activeSide?.id}
              activeTabId={activeTab?.id}
              dragging={!!dragging || !!remote}
              draggingSide={dragSideTab?.id}
              draggingTab={dragTab?.id}
              open={openTabs}
              reveal={openFolder}
              agents={agents}
              onSelect={(id) => {
                setSideTabId(id);
                refocus();
              }}
              onSelectTab={(side, tab) => {
                selectTab(side, tab);
                refocus();
              }}
              onNew={newBeside}
              onNewText={newTextBeside}
              newExtra={plugins.composed.actions
                .filter((a) => a.newMenu)
                .map((a) => ({ id: a.id, label: a.label, ...(a.accel ? { hint: chordLabel(a.accel) } : {}), run: () => runPluginAction(a.id) }))}
              onNewTab={(sideId) => {
                followNewTab(sideId);
                void send({ type: "CreateTab", sideTabId: sideId });
              }}
              // A row or tab opens its edit dialog (task 8.34); the empty list keeps its menu.
              onMenu={(at, target) =>
                target.kind === "list" ? setMenu({ ...at, target }) : editStatus(target.sideTabId, target.kind === "tab" ? target.tabId : undefined)
              }
              renaming={renaming?.in === "list" ? renaming.id : undefined}
              onRenameStart={(id) => setRenaming({ id, in: "list" })}
              onRename={rename}
              onStatus={(sideTabId, tabId, at) => setQuickStatus({ sideTabId, tabId, ...at })}
              onIconPick={(id, at) => setPicking({ id, ...at })}
              collapsed={sideCollapsed}
              onRenameEnd={() => {
                setRenaming(undefined);
                refocus();
              }}
            />
            <CommandsSection
              commands={shownCommands}
              enabled={!!activeSide}
              onAdd={(d) =>
                host.createCommand({
                  sideTabId: d.global ? null : (activeSide?.id ?? null),
                  name: d.name,
                  body: d.body,
                  target: d.target,
                  launcher: d.launcher && !d.global,
                })
              }
              onUpdate={(id, d) =>
                host.updateCommand(id, {
                  sideTabId: d.global ? null : (activeSide?.id ?? null),
                  name: d.name,
                  body: d.body,
                  target: d.target,
                  launcher: d.launcher && !d.global,
                })
              }
              onDelete={(id) =>
                removeLater({ kind: "command", id }, `Deleted ${commands.find((c) => c.id === id)?.name ?? "the command"}.`, () =>
                  host.deleteCommand(id),
                )
              }
              onRun={runCommand}
              statuses={statuses}
              onRestart={(id) => (activeSide ? host.restartLauncher(activeSide.id, id) : Promise.resolve())}
            />
          </aside>
          <main className={`app-main${appSettings.topTabs ? "" : " bare"}`}>
            {appSettings.topTabs && <TabBar
              decorate={activeSide ? (tabId) => decorate.tab(activeSide, tabId) : undefined}
              tabs={appSettings.topTabs ? tabs : []}
              folder={
                activeSide && activeSide.tabs.length > 1
                  ? { name: sideTabName(activeSide, docs), accent: activeSide.accent, lines: tabLines(activeSide, activeTab?.id) }
                  : undefined
              }
              activeId={activeTab?.id}
              agents={agents}
              statuses={statuses}
              builtins={statusConfig.builtins}
              docs={docs}
              subtitles={subtitles}
              activity={activity}
              renaming={renaming?.in === "bar" ? renaming.id : undefined}
              onRenameStart={(id) => setRenaming({ id, in: "bar" })}
              onRename={rename}
              onIconPick={(id, at) => setPicking({ id, ...at })}
              onEdit={(id) => activeSide && editStatus(activeSide.id, id)}
              onRenameEnd={() => {
                setRenaming(undefined);
                refocus();
              }}
              onSelect={(id) => {
                if (activeSide) selectTab(activeSide.id, id);
                refocus();
              }}
              onClose={closeTab}
              // The tab bar's + adds to the tabs it shows, so it can make a folder (task 8.79).
              onNew={() => {
                if (!activeSide) return;
                followNewTab(activeSide.id);
                void send({ type: "CreateTab", sideTabId: activeSide.id });
              }}
              onNewText={(beside) =>
                activeSide && void host.newText(activeSide.id, beside ? (focused ?? null) : null).then((d) => revealDoc(d.id), (e: unknown) => setError(String(e)))
              }
            />}
            <section className="app-pane">
              {activeTab && screen && (
                <SplitView
                  key={areas.map((x) => x.tab.id).join(" ")}
                  layout={screen}
                  onResize={(splitId, sizes) => {
                    // The borders between rows are the window's own; the rest are the core's.
                    if (!shownView || !hasSplit(shownView, splitId)) return send({ type: "ResizeSplit", splitId, sizes });
                    const next = resizeSplit(shownView, splitId, sizes);
                    if (typeof next === "string") return Promise.resolve(false);
                    changeView(() => next);
                    return Promise.resolve(true);
                  }}
                  // Tasks 8.73, 8.75: with two tabs or more on screen, each area carries its
                  // tab's name and coloured icon; a folder's tab also the folder's.
                  frame={
                    areas.length > 1 && !maxPane
                      ? (id, body) => {
                          const area = areas.find((x) => x.tab.layout.id === id);
                          if (!area) return body;
                          const { side, tab } = area;
                          const folder = side.tabs.length > 1;
                          const accent = (!folder && tab.accent) || side.accent;
                          return (
                            <div className={`view-area${tab.id === activeTab?.id ? " on" : ""}`} data-area={tab.id}>
                              <div
                                className="view-head"
                                style={accent ? ({ "--st-accent": `var(--neon-${accent})` } as React.CSSProperties) : undefined}
                                onPointerDown={() => {
                                  areaTip.hide();
                                  focusArea(tab.id, pick(panesOf(tab.layout), focusByTab[tab.id])!);
                                  refocus();
                                }}
                                // A folder's head names the folder and the tab it shows; the tip
                                // lists the folder's tabs, the shown one marked.
                                onMouseEnter={(e) =>
                                  folder
                                    ? areaTip.below(e.currentTarget, areaName(area), `folder of ${side.tabs.length}`, undefined, tabLines(side, tab.id))
                                    : areaTip.below(e.currentTarget, sideTabName(side, docs), subtitles[tab.id])
                                }
                                onMouseLeave={areaTip.hide}
                              >
                                {folder && <i className="ao-i ao-i-folder view-folder" aria-label="folder" />}
                                {folder && <span className="view-name">{sideTabName(side, docs)}</span>}
                                <span className="view-tab">
                                  <KidIcon agent={tabAgent(tab, agents)} icon={tab.icon} accent={tab.accent} act={activity[tab.id]} />
                                  <span className="view-tab-name">{tabName(tab, docs)}</span>
                                </span>
                              </div>
                              <div className="view-body">{body}</div>
                            </div>
                          );
                        }
                      : undefined
                  }
                  renderPane={(paneId, resume, slot, transcript) => {
                    const own = areaOfPane(paneId)?.tab ?? activeTab;
                    return (
                    <section
                      className={`ao-tp${multi && paneId === focused ? " focused" : ""}${dock?.pane === paneId && dock.side !== "center" ? ` dock-target${dock.small ? " too-small" : ""}` : ""}`}
                      data-pane={paneId}
                      onPointerDown={() => focusArea(own.id, paneId)}
                    >
                      {multi && (
                        <PaneHead paneId={paneId} name={paneHeadName(paneId, !!transcript)}>
                          <i className={`ao-i x1 ${transcript ? "ao-i-agent" : docOf(own.layout, paneId) ? "ao-i-doc" : "ao-i-term"}`} />
                          <span className="tp-name">{paneHeadName(paneId, !!transcript)}</span>
                          <span className="grow" />
                          <button
                            type="button"
                            className={`tp-close tp-max${maxPane === paneId ? " on" : ""}`}
                            aria-label={maxPane === paneId ? "Restore panes" : "Maximize pane"}
                            title={`${maxPane === paneId ? "Restore panes" : "Maximize pane"} (${chordLabel(withBindings(ACTIONS, keyBindings).find((a) => a.id === "maximize_pane")!.accel)})`}
                            onPointerDown={(e) => e.stopPropagation()}
                            onClick={() => {
                              focusArea(own.id, paneId);
                              if (maxPane === paneId) restore(own.id);
                              else setMaxByTab((m) => ({ ...m, [own.id]: paneId }));
                            }}
                          >
                            {maxPane === paneId ? "▣" : "□"}
                          </button>
                          <button
                            type="button"
                            className="tp-close"
                            aria-label="Close pane"
                            onPointerDown={(e) => e.stopPropagation()}
                            onClick={() => closePane(paneId)}
                          >
                            ×
                          </button>
                        </PaneHead>
                      )}
                      {appSettings.hook === "off" && labels[paneId]?.program === "claude" && (
                        <div className="ao-bar-wait resume-bar hook-off-bar" role="status">
                          <span className="w">Claude hook is off</span>
                          <span className="grow small ao-dim">no Claude status or transcript for this session</span>
                          <button
                            type="button"
                            className="ao-btn primary"
                            onClick={() => void host.setSettings({ ...appSettings, hook: "on" }).catch((e: unknown) => setError(String(e)))}
                          >
                            Turn on
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
                        {findPane === paneId && (
                          <SearchBar
                            ref={searchBar}
                            paneId={paneId}
                            onClose={() => {
                              setFindPane(undefined);
                              refocus();
                            }}
                          />
                        )}
                        {transcript ? (
                          <PluginPaneView
                            kind="transcript"
                            paneId={paneId}
                            sideTabId={sideOfTab(own.id) ?? ""}
                            source={transcript}
                            focused={focused === paneId}
                            plugins={plugins}
                            openSettings={() => {
                              setSettingsSection("plugins");
                              setSheet("settings");
                            }}
                            app={{
                              pin: (p) => {
                                const side = sideOfTab(own.id);
                                if (!side) return Promise.resolve();
                                return host.addPin({ ...p, sideTabId: side, originTabId: own.id, session: transcript, paneId: null, rowStart: null, rowEnd: null });
                              },
                              extract: (value) => startExtract(own.id, value, "transcript"),
                              savePrompt: (text) => {
                                setToSave(text);
                                setSheet("save-prompt");
                              },
                              ...(reveal?.session === transcript ? { reveal } : {}),
                              focus: () => focusArea(own.id, paneId),
                            }}
                          />
                        ) : (
                          slot
                        )}
                      </div>
                    </section>
                    );
                  }}
                />
              )}
              {!activeTab && <p className="small app-empty">No side tabs. Press + in the sidebar.</p>}
              {update && (
                <div className="ao-bar-wait resume-bar update-bar" role="status">
                  <span className="w">Terminescu {update} is ready</span>
                  <span className="grow small ao-dim">your shells keep running across the restart</span>
                  <button type="button" className="ao-btn primary" onClick={() => void host.updateRestart().catch((e: unknown) => setError(String(e)))}>
                    Restart to update
                  </button>
                  <button type="button" className="ao-btn ghost" onClick={() => setUpdate(undefined)}>
                    Later
                  </button>
                </div>
              )}
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
            sections={
              activeSide
                ? plugins.composed.panelSections.map((sec) => ({
                    key: `${sec.plugin}.${sec.id}`,
                    title: sec.title,
                    body: <sec.render sideTabId={activeSide.id} tabId={activeTab?.id} paneId={shellPane} app={{ openText }} bridge={plugins.bridge(sec.plugin)} />,
                  }))
                : undefined
            }
            mode={mode}
            onMode={changeMode}
            owner={activeSide && sideTabName(activeSide)}
            variables={shownCtx.variables}
            items={shownCtx.items}
            pins={shownCtx.pins}
            onOpenPin={openPin}
            onRemovePin={(p) => void removeLater({ kind: "pin", id: p.id }, "Removed the pin.", () => host.removePin(p.id))}
            onAddVariable={(v) => (sideId ? addVariable(sideId, { ...v, originTabId: activeTab?.id ?? null }) : Promise.resolve())}
            onChangeVariable={(name, change) => (sideId ? host.changeVariable(sideId, name, change) : Promise.resolve())}
            onInsertVariable={focused && activeTab && !transcriptOf(activeTab.layout, focused) ? insertVariable : undefined}
            onRemoveVariable={(name) => (sideId ? removeVariable(sideId, name) : Promise.resolve())}
            onEditVariable={(v) =>
              sideId && void host.openVariable(sideId, v.name).then((d) => revealDoc(d.id), (e: unknown) => setError(String(e)))
            }
            onAddItem={(kind, value) => (sideId ? host.addItem(sideId, kind, value, "pasted", activeTab?.id ?? null) : Promise.resolve())}
            onRemoveItem={(id) => host.removeItem(id)}
            files={shownCtx.files}
            onOpenFile={(f) => openDoc(f.path, false)}
            onForgetFile={(id) => host.forgetFile(id)}
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
          {dragSideTab && (
            <div className="ao-tab ghost">
              <span className="n">{sideTabName(dragSideTab, docs)}</span>
            </div>
          )}
          {dragPane && (
            <div className="ao-tab ghost">
              <span className="n">{paneSubtitle(labels[dragPane], !!agents[dragPane]) ?? "shell"}</span>
            </div>
          )}
        </DragOverlay>
        {dock && (
          <div
            className={`dock-preview${dock.side === "center" ? " center" : ""}${dock.small ? " small" : ""}`}
            data-side={dock.side}
            style={dock.side === "center" ? undefined : dockBox(dock.box, dock.side)}
          />
        )}
        <Tip tip={areas.length > 1 ? areaTip.tip : undefined} />
        {tabGap && <div className="tab-gap" style={{ left: tabGap.left - 1, top: tabGap.top + 4, height: tabGap.height - 8 }} />}
      </DndContext>
      {/* Every pane of the window keeps its terminal; the shown tabs' panes show (D33). */}
      {sideTabs.flatMap((side) =>
        side.tabs.flatMap((tab) =>
          terminalsOf(tab.layout).map((paneId) => (
            <TerminalPane
              key={paneId}
              paneId={paneId}
              tint={statuses[tab.id]?.look?.background ?? statuses[side.id]?.look?.background}
              font={termFont}
              behaviour={termBehaviour}
              scheme={scheme}
              opacity={terminalOpacity}
              onBell={ringBell}
              confirmPaste={confirmPaste}
              visible={areas.some((x) => x.tab.id === tab.id)}
              focused={paneId === focused}
              onFocus={() => focusArea(tab.id, paneId)}
              open={(cols, rows, onOutput) => host.openPane(paneId, cols, rows, onOutput)}
              openUrl={(url) => void host.openUrl(url).catch((e: unknown) => setError(String(e)))}
              onLinkClick={copyLink}
              onLinkMenu={(link, at) => setLinkMenu({ link, sideId: side.id, paneId, ...at })}
              openPath={(path, beside) =>
                void host.openPath(side.id, paneId, path, beside ? paneId : null).then((d) => revealDoc(d.id), (e: unknown) => setError(String(e)))
              }
              onTerminal={terminals && ((t) => (t ? terminals.set(paneId, t) : terminals.delete(paneId)))}
              onSelection={(how, text, rows) =>
                how === "pin" ? pinTerminal(tab.id, paneId, text, rows) : startExtract(tab.id, text, "terminal")
              }
            />
          )),
        ),
      )}
      {/* Text panes keep their editor the same way (D27). */}
      {sideTabs.flatMap((side) =>
        side.tabs.flatMap((tab) =>
          panesOf(tab.layout).flatMap((paneId) => {
            const docId = docOf(tab.layout, paneId);
            if (!docId) return [];
            const text = (o?: { mount?: unknown; bar?: ReactNode }) => (
              <TextPane
                key={paneId}
                paneId={paneId}
                docId={docId}
                visible={areas.some((x) => x.tab.id === tab.id)}
                focused={paneId === focused}
                onFocus={() => focusArea(tab.id, paneId)}
                host={host}
                onDoc={noteDoc}
                variables={side.id === sideId ? chipVars : undefined}
                // The plugin builds it with `@terminescu/ui`, the type `renderText` names.
                mount={o?.mount as MountEditor | undefined}
                bar={o?.bar}
              />
            );
            const tagged = docViews[docId];
            const view = tagged && plugins.composed.paneView(tagged.kind);
            if (!tagged || !view) return [text()];
            return [
              <view.render
                key={paneId}
                paneId={paneId}
                sideTabId={side.id}
                source={tagged.source}
                focused={paneId === focused}
                bridge={plugins.bridge(view.plugin)}
                app={{
                  pin: () => Promise.resolve(),
                  extract: (value) => startExtract(tab.id, value, "text"),
                  savePrompt: (t) => {
                    setToSave(t);
                    setSheet("save-prompt");
                  },
                  focus: () => focusArea(tab.id, paneId),
                  renderText: text,
                }}
              />,
            ];
          }),
        ),
      )}
      {extracting && (
        <ExtractDialog
          value={extracting.value}
          from={extracting.from}
          taken={ctx.side === extracting.side ? ctx.variables.map((v) => v.name) : []}
          defaults={appSettings}
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
          key={`${activeSide.id}:${statusFor}`}
          initialTarget={statusFor}
          side={activeSide}
          tab={activeTab}
          sideName={sideTabName(activeSide)}
          tabName={activeTab && tabName(activeTab)}
          statuses={statuses}
          config={statusConfig}
          onSet={setStatus}
          onSave={(side, look) => host.saveStatus(side, look).catch(report)}
          onMakeGlobal={(id) => host.makeGlobal(id).catch(report)}
          onSettings={() => {
            setSettingsSection("statuses");
            setSheet("settings");
          }}
          onClose={closeSheet}
          onRename={(id, title) =>
            void send(id === activeSide.id ? { type: "RenameSideTab", sideTabId: id, title } : { type: "RenameTab", tabId: id, title })
          }
          onPickIcon={(id, at) => setPicking({ id, ...at })}
          onAccent={(id, accent) => void send({ type: "SetAccent", targetId: id, accent })}
          // The right-click menu's actions; name, icon and status are fields above them.
          actions={(t) =>
            menuItems(
              t === "tab" && activeTab ? { kind: "tab", sideTabId: activeSide.id, tabId: activeTab.id } : { kind: "side", sideTabId: activeSide.id },
            ).filter((e) => e === "-" || !["rename", "icon", "status"].includes(e.id))
          }
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
            aria-modal="true"
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
      {sheet === "prompts" && (
        <PromptLibrary
          host={host}
          selectKey={libraryKey}
          onError={setError}
          onClose={closeSheet}
          onEditInText={(promptId) => {
            if (!activeSide) return;
            setSheet(undefined);
            void host.openPrompt(activeSide.id, promptId).then((d) => revealDoc(d.id), (e: unknown) => setError(String(e)));
          }}
        />
      )}
      {sheet === "first-run" && (
        <FirstRunSheet
          host={host}
          onError={setError}
          onClose={closeSheet}
          onPrivacy={() => setSheet("privacy")}
          hook={firstRunHook}
          setHook={setFirstRunHook}
        />
      )}
      {sheet === "about" && <AboutSheet host={host} onError={setError} onClose={closeSheet} onLicenses={() => setSheet("licenses")} />}
      {sheet === "licenses" && <LicensesSheet onClose={closeSheet} />}
      {sheet === "privacy" && <PrivacySheet onClose={appSettings.firstRunDone ? closeSheet : () => setSheet("first-run")} />}
      {(sheet === "feedback-email" || sheet === "feedback-issue") && (
        <FeedbackSheet key={sheet} host={host} route={sheet === "feedback-email" ? "email" : "issue"} onError={setError} onClose={closeSheet} />
      )}
      {sheet === "uninstall" && <UninstallSheet host={host} onError={setError} onClose={closeSheet} />}
      {sheet === "settings" && (
        <SettingsSheet
          section={settingsSection}
          onSection={setSettingsSection}
          statusConfig={statusConfig}
          host={host}
          sideTabName={(id) => {
            const side = workspace.store
              .getState()
              .workspace.windows.flatMap((w) => w.sideTabs)
              .find((x) => x.id === id);
            return side && sideTabName(withLiveNames([side], focusByTab, labels, agents)[0]!);
          }}
          onError={setError}
          onClose={closeSheet}
          hiddenCommands={removing.flatMap((r) => (r.kind === "command" ? [r.id] : []))}
          onDeleteCommand={(c) => removeLater({ kind: "command", id: c.id }, `Deleted ${c.name}.`, () => host.deleteCommand(c.id))}
          plugins={plugins.infos}
          pluginPages={plugins.composed.settingsPages.map((p) => ({ plugin: p.plugin, body: <p.render bridge={plugins.bridge(p.plugin)} /> }))}
          onPluginSwitch={(id, on) => void host.setPluginEnabled(id, on).catch((e: unknown) => setError(String(e)))}
        />
      )}
      {picking &&
        (() => {
          const side = sideTabs.find((sd) => sd.id === picking.id);
          const tab = side ? undefined : sideTabs.flatMap((sd) => sd.tabs).find((t) => t.id === picking.id);
          if (!side && !tab) return null;
          return (
            <IconPicker
              x={picking.x}
              y={picking.y}
              name={side ? sideTabName(side, docs) : tabName(tab!, docs)}
              current={(side ?? tab)!.icon}
              onPick={(icon) => void send({ type: "SetIcon", targetId: picking.id, icon })}
              onClose={() => {
                setPicking(undefined);
                // Picked from the edit dialog: the keyboard goes back to it, not the terminal.
                const dialog = sheet ? document.querySelector<HTMLElement>(".ao-frame[role=dialog]") : null;
                if (dialog) dialog.focus();
                else refocus();
              }}
            />
          );
        })()}
      {quickStatus &&
        (() => {
          const side = sideTabs.find((sd) => sd.id === quickStatus.sideTabId);
          const owner = quickStatus.tabId ? side?.tabs.find((t) => t.id === quickStatus.tabId) : side;
          if (!side || !owner) return null;
          const auto = statuses[owner.id]?.auto ?? "idle";
          return (
            <QuickStatus
              x={quickStatus.x}
              y={quickStatus.y}
              name={owner === side ? sideTabName(side, docs) : tabName(owner as Tab, docs)}
              current={owner.status}
              auto={chipOf({ auto, waiting: false, source: "auto" }, statusConfig.builtins)}
              local={statusConfig.statuses.filter((st) => st.sideTabId === side.id)}
              global={statusConfig.statuses.filter((st) => st.sideTabId === null)}
              onSet={(st) => void setStatus(owner.id, st)}
              onEdit={() => editStatus(side.id, quickStatus.tabId)}
              onClose={() => {
                setQuickStatus(undefined);
                refocus();
              }}
            />
          );
        })()}
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          label={menu.target.kind === "side" ? "Side tab" : menu.target.kind === "list" ? "Side tabs" : "Tab"}
          items={menuItems(menu.target)}
          onClose={() => setMenu(undefined)}
        />
      )}
      {asking && <AskDialog key={asking.n} question={asking.question} onAnswer={(a) => answerAsk(asking.n, a)} />}
      {closingSide && (
        <div className="sheet-back" onPointerDown={() => setClosingSide(undefined)}>
          <div
            className="ao-frame close-dialog"
            role="alertdialog"
            aria-modal="true"
            aria-label="Close side tab"
            onPointerDown={(e) => e.stopPropagation()}
            onKeyDown={(e) => {
              if (e.key !== "Escape") return;
              setClosingSide(undefined);
              refocus();
            }}
          >
            <div className="ao-frame-body">
              <p>
                Close {sideTabName(closingSide, docs)} and its {closingSide.tabs.length === 1 ? "tab" : `${closingSide.tabs.length} tabs`}?
              </p>
              <p className="small ao-dim">Its shells end, and its context goes with it: variables, pins, links, files and its own commands.</p>
              <div className="close-actions">
                <span className="grow" />
                <button
                  type="button"
                  className="ao-btn ghost"
                  onClick={() => {
                    setClosingSide(undefined);
                    refocus();
                  }}
                >
                  Cancel
                </button>
                <button type="button" className="ao-btn primary" autoFocus onClick={() => closeSide(closingSide)}>
                  Close side tab
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
      {closing && (
        <div className="sheet-back" onPointerDown={() => void finishClose("cancel")}>
          <div
            className="ao-frame close-dialog"
            role="alertdialog"
            aria-modal="true"
            aria-label="Unsaved changes"
            onPointerDown={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.key === "Escape" && void finishClose("cancel")}
          >
            <div className="ao-frame-body">
              <p>
                {closing.titles.length === 1
                  ? `Save changes to ${closing.titles[0]}?`
                  : `Save changes to ${closing.titles.length} documents: ${closing.titles.join(", ")}?`}
              </p>
              <p className="small ao-dim">Unsaved edits are lost when the pane closes.</p>
              <div className="close-actions">
                <button type="button" className="ao-btn ghost" onClick={() => void finishClose("discard")}>
                  Don't save
                </button>
                <span className="grow" />
                <button type="button" className="ao-btn ghost" onClick={() => void finishClose("cancel")}>
                  Cancel
                </button>
                <button type="button" className="ao-btn primary" autoFocus onClick={() => void finishClose("save")}>
                  Save
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
      {closingRunning && (
        <div className="sheet-back" onPointerDown={() => finishCloseRunning(false)}>
          <div
            className="ao-frame close-dialog"
            role="alertdialog"
            aria-modal="true"
            aria-label="Close a running program"
            onPointerDown={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.key === "Escape" && finishCloseRunning(false)}
          >
            <div className="ao-frame-body">
              <p>Close while {closingRunning.names.join(", ")} {closingRunning.names.length === 1 ? "is" : "are"} running?</p>
              <p className="small ao-dim">Closing ends the shell and what runs in it.</p>
              <div className="close-actions">
                <span className="grow" />
                <button type="button" className="ao-btn ghost" onClick={() => finishCloseRunning(false)}>
                  Cancel
                </button>
                <button type="button" className="ao-btn primary" autoFocus onClick={() => finishCloseRunning(true)}>
                  Close
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
      {pasteAsk && (
        <PasteDialog
          text={pasteAsk.text}
          reason={pasteAsk.reason}
          onPaste={() => {
            pasteAsk.answer(true);
            setPasteAsk(undefined);
          }}
          onCancel={() => {
            pasteAsk.answer(false);
            setPasteAsk(undefined);
          }}
        />
      )}
      {merging && (
        <MergeDialog
          fromName={sideTabName(merging.from, docs)}
          intoName={sideTabName(merging.into, docs)}
          clashes={merging.clashes}
          onMerge={(r) => finishMerge(merging.from, merging.into, r, merging.after)}
          onCancel={() => {
            setMerging(undefined);
            refocus();
          }}
        />
      )}
      {linkTip && (
        <div key={linkTip.key} className="px-tip link-tip" role="status" style={{ left: linkTip.x + 10, top: linkTip.y + 14 }}>
          {linkTip.text}
        </div>
      )}
      {linkMenu && (
        <ContextMenu
          x={linkMenu.x}
          y={linkMenu.y}
          label={linkMenu.link.kind === "url" ? "Link" : linkMenu.link.kind === "path" ? "Path" : "Email address"}
          items={linkActions(linkMenu.link.kind).map((a) => ({ id: a.id, label: a.label, run: () => runLink(a.id, linkMenu) }))}
          onClose={() => {
            setLinkMenu(undefined);
            refocus();
          }}
        />
      )}
      {editMenu && (
        <ContextMenu
          x={editMenu.x}
          y={editMenu.y}
          label={{ term: "Terminal", text: "Text", field: "Field", selection: "Selection" }[editMenu.kind]}
          items={editItems(editMenu)}
          onClose={() => {
            const el = editMenu.el;
            setEditMenu(undefined);
            setTimeout(() => el.focus());
          }}
        />
      )}
      {toast && (
        <div className="ao-frame toast" role="status">
          <span className="small">{toast.text}</span>
          <button type="button" className="ao-btn ghost" onClick={() => endToast(true)}>
            Undo
          </button>
        </div>
      )}
      {findingAll && (
        <FindAllSheet
          panes={allTerminals}
          onPick={pickFound}
          onClose={() => {
            setFindingAll(false);
            refocus();
          }}
        />
      )}
      {finder && (
        <FileFinder
          list={finder.list}
          onOpen={openDoc}
          onBrowse={() => browse(finder.list?.root)}
          onClose={() => {
            setFinder(undefined);
            refocus();
          }}
        />
      )}
      {sending !== undefined && (
        <Palette
          items={sendItems}
          onClose={() => {
            setSending(undefined);
            refocus();
          }}
        />
      )}
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

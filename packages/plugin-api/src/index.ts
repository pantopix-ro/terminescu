// The UI half of a plugin (add-plugin-architecture D7). A plugin declares what it adds; the
// registry composes every enabled plugin's contributions in plugin order, and the app's
// components take the composed lists as props, knowing no plugin.

import type { ComponentType, ReactNode } from "react";
import { normalAccel, RESERVED, type ActionDef, type KeyBindings, type PaneViewTag, type Decoration, type DecorationSlot, type DecorationTone, type Id } from "@terminescu/host";

/** What a plugin's UI parts call through: its own Rust half and its own events. */
export interface PluginBridge {
  call<T = unknown>(method: string, args?: unknown): Promise<T>;
  /** Returns the unsubscribe. */
  on(event: string, listener: (payload: unknown) => void): () => void;
}

/** One thing a row or a tab shows for a plugin: a chip, a subtitle part, a badge or a meter. */
export interface RenderItem {
  plugin: string;
  slot: DecorationSlot;
  key: string;
  text: string;
  tone: DecorationTone;
  tip?: string;
}

export interface RowView {
  id: Id;
  tabIds: readonly Id[];
  /** The tab the row shows now. */
  shownTabId?: Id;
}

export interface TabView {
  id: Id;
  sideTabId: Id;
  paneIds: readonly Id[];
}

/**
 * A decorator gets the items the plugins before it produced, the target and this plugin's own
 * decorations, and returns the items to show (Decorator, D7). It may add or reorder its own
 * items; the registry keeps every other plugin's items whatever it returns.
 */
export type Decorator<V> = (items: readonly RenderItem[], view: V, mine: readonly Decoration[]) => RenderItem[];

/** A pane view over a document, such as a diff tab: what it shows on the text it edits. The
 * document keeps it, so the tab draws the same view after a restart. */
export type TextView = PaneViewTag;

/** What the app does for a panel section (Facade). */
export interface SectionApp {
  /** Opens the file beside the focused pane, drawn by the pane view `view` when given. */
  openText(path: string, view?: TextView): Promise<void>;
}

export interface PanelSectionProps {
  sideTabId: Id;
  app?: SectionApp;
  /** The row's shown tab. */
  tabId?: Id;
  /** That tab's shell pane: the focused one, else the one focused last, else the first. */
  paneId?: Id;
  bridge: PluginBridge;
  /** Hides the section's head and body, or shows them again; the body stays mounted, so it can
   * show itself when its answer changes. Stable for the section (polish-claude-insights D2). */
  setHidden?(hidden: boolean): void;
}

export interface PanelSection {
  id: string;
  title: string;
  render: ComponentType<PanelSectionProps>;
}

/** Text of a pane view kept in the row's context: an entry, or `[start, end)` of its text. */
export interface PanePin {
  entryId: string;
  exact: string;
  prefix: string | null;
  suffix: string | null;
  start: number | null;
  end: number | null;
}

/** A pin of this pane to scroll to and highlight; a new `nonce` reveals it again. */
export interface PaneReveal {
  entryId: string;
  exact: string;
  prefix: string | null;
  suffix: string | null;
  start: number | null;
  nonce: number;
}

/** What the app does for a pane view (Facade): the row's context and the app's dialogs. */
export interface PaneApp {
  pin(pin: PanePin): Promise<unknown>;
  /** Opens the extract dialog for the text. */
  extract(value: string): void;
  /** Opens Save as prompt with the text. */
  savePrompt(text: string): void;
  /** The pin the person opened, when it is in this pane. */
  reveal?: PaneReveal;
  /** The pane took the focus. */
  focus(): void;
  /**
   * The app's own text pane over the leaf's document, for a view that decorates it (D6 of
   * add-git-plugin). `mount` is `@terminescu/ui`'s `MountEditor`; `bar` goes above the editor.
   * Present only for a view the app opened over a document. `readOnly` shows the text
   * read-only, as a deleted file's diff (polish-git-plugin D7).
   */
  renderText?(options?: { mount?: unknown; bar?: ReactNode; readOnly?: boolean }): ReactNode;
}

export interface PaneViewProps {
  paneId: Id;
  sideTabId: Id;
  /** What the leaf names, such as a transcript's session. */
  source: string;
  bridge: PluginBridge;
  app: PaneApp;
  /** True while the pane has the focus. */
  focused: boolean;
}

/** A kind of pane leaf the plugin draws, such as `transcript` (D9). */
export interface PaneViewDef {
  kind: string;
  render: ComponentType<PaneViewProps>;
}

/** A question the app asks for a plugin, as a dialog: a message, text fields and buttons. */
export interface Question {
  title: string;
  text?: string;
  /** A line in the terminal font, such as the command the answer runs. */
  detail?: string;
  fields?: readonly { id: string; label: string; value?: string; placeholder?: string }[];
  /** A list to pick one item from; the answer's `values.pick` is its id, the first by default. */
  pick?: { label: string; items: readonly { id: string; label: string; detail?: string }[] };
  /** The first is the default and answers Enter; `danger` paints it red. */
  choices: readonly { id: string; label: string; danger?: boolean }[];
}

/** The button picked, or null for Escape and a click outside; the fields' text by id. */
export interface Answer {
  choice: string | null;
  values: Record<string, string>;
}

/** Asks the person, one question at a time. */
export type Ask = (question: Question) => Promise<Answer>;

export interface ActionContext {
  windowId: Id;
  sideTabId?: Id;
  /** The focused pane, of any kind. */
  paneId?: Id;
  /** The selected row's shell: the focused one, else the one focused last, else the first. */
  shellPaneId?: Id;
  bridge: PluginBridge;
  ask: Ask;
  /** Opens an http or https link in the person's browser. */
  openUrl(url: string): Promise<void>;
}

/** A row about to close, after the person confirmed the close. */
export interface RowCloseContext {
  sideTabId: Id;
  /** The row's name as the sidebar shows it. */
  title: string;
  bridge: PluginBridge;
  ask: Ask;
}

/**
 * Runs before a row closes, in plugin order (Chain of Responsibility). False keeps the row
 * open and stops the chain; a throw keeps it open too, and the app shows the error.
 */
export type BeforeRowClose = (cx: RowCloseContext) => Promise<boolean>;

export interface PluginAction {
  /** Unique within the plugin; the app's id is `plugin.id`. */
  id: string;
  label: string;
  /** The menu it shows in; the palette has every action. */
  menu?: string;
  /** Asked for, as actions.json spells it; the app or the person may already use it. */
  accel?: string;
  /** Also in the sidebar's + menu, after Terminal and Text. */
  newMenu?: boolean;
  /** Also in a row's right-click menu; it runs on that row, with that row's shell. */
  rowMenu?: boolean;
  /** Shown, in menus and the palette, only while the plugin raises this flag (`UiPlugin.flags`). */
  when?: string;
  run(cx: ActionContext): void | Promise<void>;
}

export interface SettingsPageProps {
  bridge: PluginBridge;
}

export interface UiPlugin {
  /** The Rust plugin's id. */
  id: string;
  name: string;
  rowDecorators?: readonly Decorator<RowView>[];
  tabDecorators?: readonly Decorator<TabView>[];
  panelSections?: readonly PanelSection[];
  paneViews?: readonly PaneViewDef[];
  actions?: readonly PluginAction[];
  beforeRowClose?: BeforeRowClose;
  /**
   * Follows what the plugin's actions need, such as `github` while `gh` is logged in: calls
   * `set` with the raised flags each time they change. Runs while the plugin is enabled;
   * returns the stop.
   */
  flags?: (bridge: PluginBridge, set: (flags: readonly string[]) => void) => () => void;
  settingsPage?: ComponentType<SettingsPageProps>;
  /** A bar above each terminal pane, such as an agent's resume offer (harden-plugin-host D7). */
  paneBar?: ComponentType<PaneBarProps>;
  /**
   * Runs in each window while the plugin is enabled, to answer the Rust half's events with the
   * app's dialog, such as a worktree left behind (polish-git-plugin D6). Returns the stop.
   */
  watch?: (cx: WatchContext) => () => void;
}

/** What a plugin's watcher gets: its window, its bridge and the app's dialog. */
export interface WatchContext {
  windowId: Id;
  bridge: PluginBridge;
  ask: Ask;
}

/** What a pane bar can do in the app (Facade). */
export interface PaneBarApp {
  /** Types the line that resumes the pane's offered session. */
  resume(): void;
  /** Drops the pane's resume offer. */
  dismiss(): void;
  /** The agent hook setting (Settings › Shell). */
  hookOn: boolean;
  setHook(on: boolean): void;
}

export interface PaneBarProps {
  paneId: Id;
  /** The program the pane runs now, from its label. */
  program?: string;
  /** The session the pane offers to resume after a restart. */
  resume?: string;
  app: PaneBarApp;
}

export function definePlugin(plugin: UiPlugin): UiPlugin {
  return plugin;
}

const item = (d: Decoration): RenderItem => ({
  plugin: d.plugin,
  slot: d.slot,
  key: d.key,
  text: d.text,
  tone: d.tone,
  ...(d.tip === undefined ? {} : { tip: d.tip }),
});

/**
 * A plugin with no row decorator shows its decorations on the row as they are. A row of one tab
 * is that tab in the sidebar, so it shows the tab's too.
 */
export const showOwnRow: Decorator<RowView> = (items, row, mine) => [
  ...items,
  ...mine
    .filter((d) => (d.target.kind === "sideTab" && d.target.id === row.id) || (row.tabIds.length === 1 && d.target.kind === "tab" && d.target.id === row.tabIds[0]))
    .map(item),
];

/** A plugin with no tab decorator shows its decorations on the tab as they are. */
export const showOwnTab: Decorator<TabView> = (items, tab, mine) => [...items, ...mine.filter((d) => d.target.kind === "tab" && d.target.id === tab.id).map(item)];

/** Turns decorations into items: a helper for a decorator that shows another target's. */
export const toItems = (ds: readonly Decoration[]): RenderItem[] => ds.map(item);

export interface ComposedAction extends ActionDef {
  plugin: string;
  /** The chord the plugin asks for; empty when it asks for none. */
  wantAccel: string;
  /** The chord the plugin asked for, when it lost it to the app, the person or another plugin. */
  lostAccel?: string;
  /** The person set this action's chord in Settings › Keys (empty is off). */
  bound: boolean;
  newMenu: boolean;
  rowMenu: boolean;
  run: PluginAction["run"];
}

export interface Composed {
  rowItems(row: RowView): RenderItem[];
  tabItems(tab: TabView): RenderItem[];
  panelSections: readonly (PanelSection & { plugin: string })[];
  /** The pane view of `kind`, with its plugin; none when no enabled plugin draws it. */
  paneView(kind: string): (PaneViewDef & { plugin: string }) | undefined;
  /** Kinds an installed but disabled plugin draws, for the missing-view notice. */
  offKinds: ReadonlySet<string>;
  actions: readonly ComposedAction[];
  /** Every action of the enabled plugins, hidden ones too, for Settings › Keys. */
  keyActions: readonly (ComposedAction & { pluginName: string; hidden: boolean })[];
  /** The enabled plugins' close hooks, in plugin order. */
  beforeRowClose: readonly { plugin: string; run: BeforeRowClose }[];
  settingsPages: readonly { plugin: string; name: string; render: ComponentType<SettingsPageProps> }[];
  /** The enabled plugins' pane bars, in plugin order. */
  paneBars: readonly { plugin: string; render: ComponentType<PaneBarProps> }[];
  /** The enabled plugins' watchers, in plugin order. */
  watchers: readonly { plugin: string; watch: (cx: WatchContext) => () => void }[];
}

export interface ComposeInput {
  /** In plugin order. */
  plugins: readonly UiPlugin[];
  enabled: ReadonlySet<string>;
  decorations: readonly Decoration[];
  /** The app's actions with the person's chords in place. */
  appActions: readonly ActionDef[];
  /** Each plugin's raised flags; an action whose `when` is not raised is left out. */
  flags?: ReadonlyMap<string, ReadonlySet<string>>;
  /** The person's chords from Settings › Keys; plugin actions read `plugin.action`. */
  bindings?: KeyBindings;
}

/**
 * Composes the enabled plugins (D7). Decorators run in plugin order, each over the items so
 * far; a plugin's items keep only what that plugin produced, so one plugin cannot drop
 * another's. The person's chords come first, then each plugin's in plugin order; a chord the
 * app, macOS, the person or an earlier plugin holds goes, and the action stays, chordless. A
 * hidden action holds its chord too, so a flag rising never moves one
 * (add-configurable-shortcuts D4).
 */
export function compose({ plugins, enabled, decorations, appActions, flags, bindings = {} }: ComposeInput): Composed {
  const on = plugins.filter((p) => enabled.has(p.id));
  const mine = new Map(on.map((p) => [p.id, decorations.filter((d) => d.plugin === p.id)]));
  const run = <V>(view: V, pick: (p: UiPlugin) => readonly Decorator<V>[]) =>
    on.reduce<RenderItem[]>((items, p) => {
      const others = items.filter((i) => i.plugin !== p.id);
      const out = pick(p).reduce<RenderItem[]>((acc, d) => d(acc, view, mine.get(p.id) ?? []), items);
      return [...others, ...out.filter((i) => i.plugin === p.id)];
    }, []);

  const taken = new Set([...appActions.map((a) => normalAccel(a.accel)), ...RESERVED.map((r) => normalAccel(r.accel))].filter((a) => a));
  const isCmd = (n: string) => n !== "" && /(^|\+)Cmd(\+|$)/.test(n);
  const all = on.flatMap((p) => (p.actions ?? []).map((a) => ({ p, a, id: `${p.id}.${a.id}` })));
  const chord = new Map<string, string>();
  // The person's chords first, then the plugins' in plugin order.
  for (const { id } of all)
    if (Object.hasOwn(bindings, id)) {
      const want = normalAccel(bindings[id]!);
      if (isCmd(want) && !taken.has(want)) {
        taken.add(want);
        chord.set(id, bindings[id]!);
      }
    }
  for (const { a, id } of all)
    if (!Object.hasOwn(bindings, id) && a.accel) {
      const want = normalAccel(a.accel);
      if (isCmd(want) && !taken.has(want)) {
        taken.add(want);
        chord.set(id, a.accel);
      }
    }
  const keyActions = all.map(({ p, a, id }) => {
    const bound = Object.hasOwn(bindings, id);
    const accel = chord.get(id) ?? "";
    const want = bound ? bindings[id]! : (a.accel ?? "");
    return {
      id,
      label: a.label,
      menu: a.menu ?? "Plugins",
      accel,
      wantAccel: a.accel ?? "",
      bound,
      plugin: p.id,
      pluginName: p.name,
      hidden: a.when !== undefined && !flags?.get(p.id)?.has(a.when),
      newMenu: a.newMenu === true,
      rowMenu: a.rowMenu === true,
      run: a.run,
      ...(want && !accel ? { lostAccel: want } : {}),
    };
  });
  const actions: ComposedAction[] = keyActions.filter((k) => !k.hidden).map(({ pluginName: _n, hidden: _h, ...rest }) => rest);

  const views = new Map<string, PaneViewDef & { plugin: string }>();
  for (const p of on) for (const v of p.paneViews ?? []) if (!views.has(v.kind)) views.set(v.kind, { ...v, plugin: p.id });
  const offKinds = new Set(plugins.filter((p) => !enabled.has(p.id)).flatMap((p) => (p.paneViews ?? []).map((v) => v.kind)).filter((k) => !views.has(k)));

  return {
    rowItems: (row) => run(row, (p) => p.rowDecorators ?? [showOwnRow]),
    tabItems: (tab) => run(tab, (p) => p.tabDecorators ?? [showOwnTab]),
    panelSections: on.flatMap((p) => (p.panelSections ?? []).map((s) => ({ ...s, plugin: p.id }))),
    paneView: (kind) => views.get(kind),
    offKinds,
    actions,
    keyActions,
    beforeRowClose: on.flatMap((p) => (p.beforeRowClose ? [{ plugin: p.id, run: p.beforeRowClose }] : [])),
    settingsPages: on.flatMap((p) => (p.settingsPage ? [{ plugin: p.id, name: p.name, render: p.settingsPage }] : [])),
    paneBars: on.flatMap((p) => (p.paneBar ? [{ plugin: p.id, render: p.paneBar }] : [])),
    watchers: on.flatMap((p) => (p.watch ? [{ plugin: p.id, watch: p.watch }] : [])),
  };
}

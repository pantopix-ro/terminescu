// The UI half of a plugin (add-plugin-architecture D7). A plugin declares what it adds; the
// registry composes every enabled plugin's contributions in plugin order, and the app's
// components take the composed lists as props, knowing no plugin.

import type { ComponentType } from "react";
import { normalAccel, RESERVED, type ActionDef, type Decoration, type DecorationSlot, type DecorationTone, type Id } from "@terminescu/host";

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

export interface PanelSectionProps {
  sideTabId: Id;
  /** The row's shown tab. */
  tabId?: Id;
  /** That tab's shell pane: the focused one, else the one focused last, else the first. */
  paneId?: Id;
  bridge: PluginBridge;
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

export interface ActionContext {
  windowId: Id;
  sideTabId?: Id;
  paneId?: Id;
  bridge: PluginBridge;
}

export interface PluginAction {
  /** Unique within the plugin; the app's id is `plugin.id`. */
  id: string;
  label: string;
  /** The menu it shows in; the palette has every action. */
  menu?: string;
  /** Asked for, as actions.json spells it; the app or the person may already use it. */
  accel?: string;
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
  settingsPage?: ComponentType<SettingsPageProps>;
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
  /** The chord the plugin asked for, when it lost it to the app or the person. */
  lostAccel?: string;
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
  settingsPages: readonly { plugin: string; name: string; render: ComponentType<SettingsPageProps> }[];
}

export interface ComposeInput {
  /** In plugin order. */
  plugins: readonly UiPlugin[];
  enabled: ReadonlySet<string>;
  decorations: readonly Decoration[];
  /** The app's actions with the person's chords in place. */
  appActions: readonly ActionDef[];
}

/**
 * Composes the enabled plugins (D7). Decorators run in plugin order, each over the items so
 * far; a plugin's items keep only what that plugin produced, so one plugin cannot drop
 * another's. A chord the app, macOS or an earlier plugin holds goes: the action stays, chordless.
 */
export function compose({ plugins, enabled, decorations, appActions }: ComposeInput): Composed {
  const on = plugins.filter((p) => enabled.has(p.id));
  const mine = new Map(on.map((p) => [p.id, decorations.filter((d) => d.plugin === p.id)]));
  const run = <V>(view: V, pick: (p: UiPlugin) => readonly Decorator<V>[]) =>
    on.reduce<RenderItem[]>((items, p) => {
      const others = items.filter((i) => i.plugin !== p.id);
      const out = pick(p).reduce<RenderItem[]>((acc, d) => d(acc, view, mine.get(p.id) ?? []), items);
      return [...others, ...out.filter((i) => i.plugin === p.id)];
    }, []);

  const taken = new Set([...appActions.map((a) => normalAccel(a.accel)), ...RESERVED.map((r) => normalAccel(r.accel))].filter((a) => a));
  const actions: ComposedAction[] = [];
  for (const p of on)
    for (const a of p.actions ?? []) {
      const want = a.accel ? normalAccel(a.accel) : "";
      const free = want !== "" && /(^|\+)Cmd(\+|$)/.test(want) && !taken.has(want);
      if (free) taken.add(want);
      actions.push({
        id: `${p.id}.${a.id}`,
        label: a.label,
        menu: a.menu ?? "Plugins",
        accel: free ? a.accel! : "",
        plugin: p.id,
        run: a.run,
        ...(a.accel && !free ? { lostAccel: a.accel } : {}),
      });
    }

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
    settingsPages: on.flatMap((p) => (p.settingsPage ? [{ plugin: p.id, name: p.name, render: p.settingsPage }] : [])),
  };
}

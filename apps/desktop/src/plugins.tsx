// The plugins' UI halves and their composition (add-plugin-architecture D7, D8). The app reads
// only the composed lists; which plugins exist is decided here and in the Rust build.

import { useEffect, useMemo, useState } from "react";
import type { ActionDef, Decoration, Host, PluginInfo } from "@terminescu/host";
import type { FakePlugin } from "@terminescu/host/mock";
import { compose, definePlugin, type Composed, type PluginBridge, type UiPlugin } from "@terminescu/plugin-api";
import { claude } from "@terminescu/plugin-claude";

/** The UI halves built into the app, in plugin order. */
const BUILT_IN: readonly UiPlugin[] = [claude];

/** A bridge to one plugin's Rust half. */
export function bridgeOf(host: Host, plugin: string): PluginBridge {
  return {
    call: (method, args) => host.pluginCall(plugin, method, args),
    on(event, listener) {
      let off: (() => void) | undefined;
      let gone = false;
      void host
        .onPluginEvent(plugin, (e, payload) => e === event && listener(payload))
        .then((o) => (gone ? o() : (off = o)));
      return () => {
        gone = true;
        off?.();
      };
    },
  };
}

/** The plugins in plugin order: a Rust half without a UI half still shows its decorations. */
function uiPlugins(infos: readonly PluginInfo[], halves: readonly UiPlugin[]): UiPlugin[] {
  return infos.map((i) => halves.find((p) => p.id === i.id) ?? definePlugin({ id: i.id, name: i.name }));
}

export interface Plugins {
  infos: PluginInfo[];
  composed: Composed;
  /** The name of the plugin that draws `kind`, when it is installed but off. */
  offOwner(kind: string): string | undefined;
  bridge(plugin: string): PluginBridge;
}

/** Follows the plugin list and the decorations and composes the enabled plugins (D7). */
export function usePlugins(host: Host, appActions: readonly ActionDef[], halves: readonly UiPlugin[] = HALVES): Plugins {
  const [infos, setInfos] = useState<PluginInfo[]>([]);
  const [decorations, setDecorations] = useState<Decoration[]>([]);
  useEffect(() => {
    let gone = false;
    const offs: (() => void)[] = [];
    const keep = (p: Promise<() => void>) => void p.then((o) => (gone ? o() : offs.push(o)), () => undefined);
    keep(host.onPlugins((p) => !gone && setInfos(p)));
    keep(host.onDecorations((d) => !gone && setDecorations(d)));
    void host.plugins().then((p) => !gone && setInfos(p), () => undefined);
    void host.decorations().then((d) => !gone && setDecorations(d), () => undefined);
    return () => {
      gone = true;
      offs.forEach((o) => o());
    };
  }, [host]);
  const plugins = useMemo(() => uiPlugins(infos, halves), [infos, halves]);
  const enabled = useMemo(() => new Set(infos.filter((i) => i.enabled && !i.paused).map((i) => i.id)), [infos]);
  const composed = useMemo(() => compose({ plugins, enabled, decorations, appActions }), [plugins, enabled, decorations, appActions]);
  // The native menu (and the mock's chords) follow the composed actions.
  const menuKey = composed.actions.map((a) => `${a.id}|${a.label}|${a.menu}|${a.accel}`).join("\n");
  useEffect(() => {
    void host.setPluginActions(composed.actions.map(({ id, label, menu, accel }) => ({ id, label, menu, accel }))).catch(() => undefined);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [host, menuKey]);
  const bridges = useMemo(() => new Map<string, PluginBridge>(), [host]);
  return {
    infos,
    composed,
    offOwner: (kind) => (composed.offKinds.has(kind) ? plugins.find((p) => p.paneViews?.some((v) => v.kind === kind))?.name : undefined),
    bridge(plugin) {
      let b = bridges.get(plugin);
      if (!b) bridges.set(plugin, (b = bridgeOf(host, plugin)));
      return b;
    },
  };
}

// ---- The fake plugin: the browser build only, with `?plugins=fake`, for e2e/plugins.spec.ts.

const fakeOn = () => typeof location !== "undefined" && new URLSearchParams(location.search).get("plugins") === "fake";

/** The fake plugin's Rust half, played by the mock host. */
export function fakeRustHalves(): FakePlugin[] {
  if (!fakeOn()) return [];
  return [
    {
      id: "fake",
      name: "Fake",
      description: "Plays every slot, for the tests.",
      call(method, args, host) {
        if (method === "ping") return "pong";
        if (method === "hello") {
          const row = (args as { sideTabId?: string } | null)?.sideTabId;
          if (row) host.decorate({ plugin: "fake", target: { kind: "sideTab", id: row }, slot: "chip", key: "hello", text: "hi", tone: "good" });
          return null;
        }
        throw new Error(`${method} is not a method of this plugin`);
      },
    },
    { id: "viewer", name: "Viewer", description: "Draws transcript panes, for the tests." },
  ];
}

function FakeSection({ bridge }: { sideTabId: string; bridge: PluginBridge }) {
  const [answer, setAnswer] = useState<string>();
  return (
    <div className="fake-section small">
      <button type="button" className="ao-btn ghost" onClick={() => void bridge.call<string>("ping").then(setAnswer, (e: unknown) => setAnswer(String(e)))}>
        Ping
      </button>
      {answer && <span className="fake-answer">{answer}</span>}
    </div>
  );
}

function FakeView({ paneId }: { paneId: string }) {
  return <div className="fake-view">Fake view of {paneId}</div>;
}

const FAKE_HALVES: readonly UiPlugin[] = [
  definePlugin({
    id: "fake",
    name: "Fake",
    panelSections: [{ id: "notes", title: "Fake Notes", render: FakeSection }],
    settingsPage: () => <p className="fake-settings small">Fake settings</p>,
    actions: [
      { id: "hello", label: "Fake Hello", accel: "Cmd+Alt+J", run: (cx) => void cx.bridge.call("hello", { sideTabId: cx.sideTabId ?? null }) },
      { id: "steal", label: "Fake New Tab", accel: "Cmd+T", run: () => undefined },
    ],
  }),
  definePlugin({ id: "viewer", name: "Viewer", paneViews: [{ kind: "transcript", render: FakeView }] }),
];

/** The UI halves of this build. */
const HALVES: readonly UiPlugin[] = fakeOn() ? [...BUILT_IN, ...FAKE_HALVES] : BUILT_IN;

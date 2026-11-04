import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import type { PaneView } from "@terminescu/host";
import { useEffect, useRef, useState } from "react";
import { webglBudget } from "./webgl-budget";
import { terminalFont, xtermTheme } from "./xterm-theme";

export interface TerminalPaneProps {
  tabId: string;
  /** Hidden panes keep their terminal and keep receiving output, but render nothing. */
  visible: boolean;
  open(cols: number, rows: number, onOutput: (bytes: Uint8Array) => void): Promise<PaneView>;
  openUrl(url: string): void;
  /** Hands out the terminal, for tests. */
  onTerminal?(term: Terminal | null): void;
}

interface Live {
  term: Terminal;
  fit: FitAddon;
  view?: PaneView;
  /** Bytes written before the view existed, acknowledged once it does. */
  early: number;
  gl?: WebglAddon;
  /** The context was lost once; this pane stays on the DOM renderer. */
  glLost: boolean;
}

let fonts: Promise<unknown> | undefined;
/** xterm.js measures the cell once, so the font must be loaded first. */
function fontsReady(): Promise<unknown> {
  fonts ??= document.fonts
    ? document.fonts.load(`${terminalFont.fontSize}px "Geist Mono"`).catch(() => undefined)
    : Promise.resolve();
  return fonts;
}

const isHttp = (uri: string) => /^https?:\/\//i.test(uri);

function takeWebgl(live: Live, el: HTMLElement) {
  if (live.gl || live.glLost || !webglBudget.acquire()) return;
  try {
    const gl = new WebglAddon();
    gl.onContextLoss(() => {
      live.glLost = true;
      dropWebgl(live, el);
    });
    live.term.loadAddon(gl);
    live.gl = gl;
    el.dataset.renderer = "webgl";
  } catch {
    webglBudget.release();
  }
}

function dropWebgl(live: Live, el: HTMLElement) {
  if (!live.gl) return;
  live.gl.dispose();
  live.gl = undefined;
  webglBudget.release();
  el.dataset.renderer = "dom";
}

/**
 * One xterm.js terminal on one tab's pane (task 2.5, D33). `⌘` chords pass through to the app
 * menu; every other key goes to the shell, with Option as Meta. Links open on `⌘`-click.
 */
export function TerminalPane({ tabId, visible, open, openUrl, onTerminal }: TerminalPaneProps) {
  const ref = useRef<HTMLDivElement>(null);
  const live = useRef<Live | null>(null);
  const [ready, setReady] = useState(false);
  const callbacks = useRef({ open, openUrl, onTerminal });
  callbacks.current = { open, openUrl, onTerminal };

  useEffect(() => {
    const el = ref.current!;
    let disposed = false;
    let observer: ResizeObserver | undefined;
    let frame = 0;

    void (async () => {
      await fontsReady();
      if (disposed) return;
      const activate = (e: MouseEvent, uri: string) => {
        if (e.metaKey && isHttp(uri)) callbacks.current.openUrl(uri);
      };
      const term = new Terminal({
        ...terminalFont,
        theme: xtermTheme,
        scrollback: 10_000,
        macOptionIsMeta: true,
        linkHandler: { activate },
      });
      const fit = new FitAddon();
      term.loadAddon(fit);
      term.loadAddon(new WebLinksAddon(activate));
      // ⌘ chords belong to the app menu (D17), including copy and paste.
      term.attachCustomKeyEventHandler((e) => !e.metaKey);
      term.open(el);
      el.dataset.renderer = "dom";
      const l: Live = { term, fit, early: 0, glLost: false };
      live.current = l;
      if (el.clientWidth > 0) fit.fit();
      observer = new ResizeObserver(() => {
        cancelAnimationFrame(frame);
        frame = requestAnimationFrame(() => {
          if (!disposed && el.clientWidth > 0) fit.fit();
        });
      });
      observer.observe(el);
      callbacks.current.onTerminal?.(term);

      const ack = (n: number) => (l.view ? l.view.ack(n) : (l.early += n));
      let view: PaneView;
      try {
        view = await callbacks.current.open(term.cols, term.rows, (bytes) =>
          term.write(bytes, () => ack(bytes.length)),
        );
      } catch (e) {
        if (!disposed) term.write(`\r\n[could not open this pane: ${JSON.stringify(e)}]\r\n`);
        return;
      }
      if (disposed) return view.close();
      l.view = view;
      if (l.early) view.ack(l.early);
      term.onData((d) => view.input(d));
      term.onBinary((d) => view.input(d, true));
      term.onResize(({ cols, rows }) => view.resize(cols, rows));
      setReady(true);
    })();

    return () => {
      disposed = true;
      cancelAnimationFrame(frame);
      observer?.disconnect();
      const l = live.current;
      live.current = null;
      if (!l) return;
      l.view?.close();
      dropWebgl(l, el);
      l.term.dispose();
      callbacks.current.onTerminal?.(null);
    };
  }, [tabId]);

  useEffect(() => {
    const l = live.current;
    const el = ref.current;
    if (!l || !el) return;
    if (!visible) return dropWebgl(l, el);
    takeWebgl(l, el);
    if (el.clientWidth > 0) l.fit.fit();
    l.term.focus();
  }, [visible, ready]);

  return <div ref={ref} className="term-pane" data-tab={tabId} hidden={!visible} />;
}

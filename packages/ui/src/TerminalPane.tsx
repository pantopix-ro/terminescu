import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal, type IBufferLine } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import type { PaneView, Tint } from "@terminescu/host";
import { useEffect, useRef, useState } from "react";
import { claimPane, keyboardHeld, paneElement, releasePane } from "./pane-registry";
import { EXTRACT_EVENT, INSERT_EVENT, PIN_EVENT, type InsertDetail } from "./events";
import { pasteBytes } from "./insert";
import { findPaths } from "./paths";
import { webglBudget } from "./webgl-budget";
import { terminalFont, themeFor, type TerminalFont } from "./xterm-theme";

export interface TerminalPaneProps {
  paneId: string;
  /** Hidden panes keep their terminal and keep receiving output, but render nothing. */
  visible: boolean;
  /** The pane that takes the keys in its tab. */
  focused: boolean;
  /** The person clicked into this pane or its terminal took focus. */
  onFocus?(): void;
  open(cols: number, rows: number, onOutput: (bytes: Uint8Array) => void): Promise<PaneView>;
  openUrl(url: string): void;
  /**
   * `⌘`-click on a file path in the output; `⌥⌘`-click opens it beside the pane (D27). The
   * path is as printed: relative ones resolve against the pane's cwd in the core.
   */
  openPath?(path: string, beside: boolean): void;
  /** Hands out the terminal, for tests. */
  onTerminal?(term: Terminal | null): void;
  /**
   * `⌘⇧P` or `⌘E` with text selected: the text and its first and last buffer rows (D12). Nothing
   * selected, nothing happens.
   */
  onSelection?(how: "pin" | "extract", text: string, rows: [number, number]): void;
  /** The status background the terminal takes, live (task 8.14). None keeps the panel colour. */
  tint?: Tint | null;
  /** Font and spacing from Settings › Terminal, live (task 8.36). None keeps the defaults. */
  font?: TerminalFont;
}

/** Scrolls a terminal to a pin's rows and selects them; `detail` is `[first, last]`. */
export const REVEAL_ROWS_EVENT = "terminescu:reveal-rows";

/** The cell column of a string offset in a line from `translateToString(true)`. */
function cellOf(line: IBufferLine, offset: number): number {
  let chars = 0;
  for (let x = 0; x < line.length; x++) {
    const cell = line.getCell(x);
    if (!cell || cell.getWidth() === 0) continue;
    const n = cell.getChars().length || 1;
    if (chars + n > offset) return x;
    chars += n;
  }
  return Math.max(0, line.length - 1);
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

const fonts = new Map<string, Promise<unknown>>();
/** xterm.js measures the cell when the font changes, so the font must be loaded first. */
function fontsReady(font: TerminalFont): Promise<unknown> {
  const key = `${font.fontWeight} ${font.fontSize}px ${font.fontFamily}`;
  let ready = fonts.get(key);
  if (!ready) {
    ready = document.fonts ? document.fonts.load(key).catch(() => undefined) : Promise.resolve();
    fonts.set(key, ready);
  }
  return ready;
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

const selections = new Map<string, () => string>();

/** The selected text of the terminal on `paneId`, empty when it has none. */
export function terminalSelection(paneId: string): string {
  return selections.get(paneId)?.() ?? "";
}

/**
 * One xterm.js terminal on one pane (task 2.5, D33). It renders no React tree of its own: the
 * terminal lives in the pane's element from the registry, which the split view places. `⌘`
 * chords pass through to the app menu; every other key goes to the shell, with Option as Meta.
 * Links open on `⌘`-click.
 */
export function TerminalPane({ paneId, visible, focused, onFocus, open, openUrl, openPath, onTerminal, onSelection, tint, font = terminalFont }: TerminalPaneProps) {
  const live = useRef<Live | null>(null);
  const [ready, setReady] = useState(false);
  const callbacks = useRef({ open, openUrl, openPath, onTerminal, onFocus, onSelection });
  callbacks.current = { open, openUrl, openPath, onTerminal, onFocus, onSelection };
  const tintNow = useRef(tint);
  tintNow.current = tint;
  const fontNow = useRef(font);
  fontNow.current = font;
  const fontKey = JSON.stringify(font);

  useEffect(() => {
    const el = claimPane(paneId);
    const focusIn = () => callbacks.current.onFocus?.();
    el.addEventListener("focusin", focusIn);
    const selected = () => live.current?.term?.getSelection() ?? "";
    selections.set(paneId, selected);
    // `⌘⇧P` and `⌘E` from the app act on the terminal's selection.
    const selection = (how: "pin" | "extract") => () => {
      const term = live.current?.term;
      const pos = term?.getSelectionPosition();
      const text = term?.getSelection() ?? "";
      if (!term || !pos || !text.trim()) return;
      callbacks.current.onSelection?.(how, text, [pos.start.y, pos.end.y]);
    };
    const onPin = selection("pin");
    const onExtract = selection("extract");
    const onReveal = (e: Event) => {
      const term = live.current?.term;
      const rows = (e as CustomEvent<[number, number]>).detail;
      if (!term || !rows) return;
      const last = term.buffer.active.length - 1;
      if (rows[0] > last) return;
      term.scrollToLine(Math.max(0, rows[0] - 2));
      term.selectLines(rows[0], Math.min(rows[1], last));
    };
    // Not xterm's paste: without bracketed paste mode it turns line breaks into `\r`.
    const onInsert = (e: Event) => {
      const term = live.current?.term;
      const view = live.current?.view;
      const d = (e as CustomEvent<InsertDetail>).detail;
      if (!term || !view || !d?.text) return;
      view.input(pasteBytes(d.text, term.modes.bracketedPasteMode, d.send));
      if (!d.stay) term.focus();
    };
    el.addEventListener(PIN_EVENT, onPin);
    el.addEventListener(EXTRACT_EVENT, onExtract);
    el.addEventListener(INSERT_EVENT, onInsert);
    el.addEventListener(REVEAL_ROWS_EVENT, onReveal);
    let disposed = false;
    let observer: ResizeObserver | undefined;
    let frame = 0;

    void (async () => {
      const startFont = fontNow.current;
      await fontsReady(startFont);
      if (disposed) return;
      const activate = (e: MouseEvent, uri: string) => {
        if (e.metaKey && isHttp(uri)) callbacks.current.openUrl(uri);
      };
      const term = new Terminal({
        ...startFont,
        theme: themeFor(tintNow.current),
        scrollback: 10_000,
        macOptionIsMeta: true,
        linkHandler: { activate },
      });
      const fit = new FitAddon();
      term.loadAddon(fit);
      term.loadAddon(new WebLinksAddon(activate));
      // File paths underline on hover like links and open on `⌘`-click (D27).
      term.registerLinkProvider({
        provideLinks(y, done) {
          const line = term.buffer.active.getLine(y - 1);
          if (!line || !callbacks.current.openPath) return done(undefined);
          const text = line.translateToString(true);
          const links = findPaths(text).map((p) => ({
            range: { start: { x: cellOf(line, p.start) + 1, y }, end: { x: cellOf(line, p.end - 1) + 1, y } },
            text: p.path,
            activate: (e: MouseEvent, path: string) => {
              if (e.metaKey) callbacks.current.openPath?.(path, e.altKey);
            },
          }));
          done(links.length ? links : undefined);
        },
      });
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
        view = await callbacks.current.open(term.cols, term.rows, (bytes) => {
          if (!disposed) term.write(bytes, () => ack(bytes.length));
        });
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
      if (selections.get(paneId) === selected) selections.delete(paneId);
      disposed = true;
      cancelAnimationFrame(frame);
      observer?.disconnect();
      el.removeEventListener("focusin", focusIn);
      el.removeEventListener(PIN_EVENT, onPin);
      el.removeEventListener(EXTRACT_EVENT, onExtract);
      el.removeEventListener(INSERT_EVENT, onInsert);
      el.removeEventListener(REVEAL_ROWS_EVENT, onReveal);
      const l = live.current;
      live.current = null;
      if (l) {
        l.view?.close();
        dropWebgl(l, el);
        l.term.dispose();
        callbacks.current.onTerminal?.(null);
      }
      releasePane(paneId);
    };
  }, [paneId]);

  useEffect(() => {
    const el = paneElement(paneId);
    el.hidden = !visible;
    const l = live.current;
    if (!l) return;
    if (!visible) return dropWebgl(l, el);
    takeWebgl(l, el);
    if (el.clientWidth > 0) l.fit.fit();
  }, [paneId, visible, ready]);

  useEffect(() => {
    if (visible && focused && !keyboardHeld()) live.current?.term.focus();
  }, [visible, focused, ready]);

  // A new status recolours the running terminal at once; the slot around it follows in CSS.
  useEffect(() => {
    const el = paneElement(paneId);
    if (tint) el.dataset.tint = tint;
    else delete el.dataset.tint;
    const term = live.current?.term;
    if (term) term.options.theme = themeFor(tint);
  }, [paneId, tint, ready]);

  // Settings › Terminal applies at once: xterm.js measures the new cell, then the pane refits
  // and the shell gets its new size.
  useEffect(() => {
    let gone = false;
    const next = fontNow.current;
    void fontsReady(next).then(() => {
      const l = live.current;
      if (gone || !l) return;
      // xterm.js skips a value that did not change.
      Object.assign(l.term.options, next);
      const el = paneElement(paneId);
      if (el.clientWidth > 0 && !el.hidden) l.fit.fit();
    });
    return () => {
      gone = true;
    };
  }, [paneId, fontKey, ready]);

  return null;
}

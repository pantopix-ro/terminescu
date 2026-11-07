import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon, type ISearchOptions } from "@xterm/addon-search";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal, type IBufferLine, type ITheme } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import type { PaneView, Tint } from "@terminescu/host";
import { useEffect, useRef, useState } from "react";
import { claimPane, keyboardHeld, paneElement, releasePane } from "./pane-registry";
import { EXTRACT_EVENT, INSERT_EVENT, PIN_EVENT, type InsertDetail } from "./events";
import { pasteBytes } from "./insert";
import { findPaths } from "./paths";
import { findEmails, type TerminalLink } from "./links";
import { webglBudget } from "./webgl-budget";
import { colors } from "./generated/colors";
import { pasteRisk } from "./paste";
import { attachPromptMarks, type PromptMarks } from "./prompt-marks";
import type { Rows } from "./find-all";
import { terminalBehaviour, terminalBehaviourOptions, terminalFont, themeFor, type TerminalBehaviourSettings, type TerminalFont } from "./xterm-theme";

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
  /** Cursor, history, Option keys, bell, clipboard and contrast, live (task 8.42). */
  behaviour?: TerminalBehaviourSettings;
  /** The colour scheme (task 8.47); a status tint still replaces its background. */
  scheme?: ITheme;
  /** 30 to 100; below 100 the terminal's background lets the desktop through. */
  opacity?: number;
  /** The shell rang the bell and the setting is visual or sound. */
  onBell?(kind: "visual" | "sound"): void;
  /** A plain click on a link (task 8.49): the app copies it and says so at `at`. */
  onLinkClick?(link: TerminalLink, at: { x: number; y: number }): void;
  /** A right click on a link: the app shows the menu for its kind. */
  onLinkMenu?(link: TerminalLink, at: { x: number; y: number }): void;
  /** A risky paste waits for this; true pastes. Without it the paste goes through. */
  confirmPaste?(text: string, reason: string): Promise<boolean>;
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
  search: SearchAddon;
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

/** What the find bar asks of a pane's terminal (task 8.38). */
export interface FindOptions {
  caseSensitive: boolean;
  wholeWord: boolean;
  regex: boolean;
}
export interface FindResults {
  /** The active match, from 0; -1 when there is none or past the highlight limit. */
  index: number;
  count: number;
}
interface Finder {
  find(query: string, opts: FindOptions, back: boolean, incremental: boolean): boolean;
  clear(): void;
  listen(cb: (r: FindResults) => void): () => void;
}
const finders = new Map<string, Finder>();
/** The find bar's handle on a pane's terminal; none before the terminal exists. */
export const finderOf = (paneId: string): Finder | undefined => finders.get(paneId);

// Matches use the selection's blue and the active one amber; the scrollbar shows both.
const MATCHES: ISearchOptions["decorations"] = {
  matchBackground: colors.selection,
  matchOverviewRuler: colors.selection,
  activeMatchBackground: colors.amber,
  activeMatchColorOverviewRuler: colors.amber,
};

const buffers = new Map<string, Terminal>();
/** The history and screen of `paneId`'s terminal for find in all panes (task 8.46). */
export function terminalRows(paneId: string): Rows | undefined {
  const term = buffers.get(paneId);
  if (!term) return undefined;
  // The normal buffer: a full-screen program's alternate screen has no history worth finding.
  const b = term.buffer.normal;
  return {
    length: b.length,
    row: (i) => {
      const line = b.getLine(i);
      return line && { text: line.translateToString(true), wrapped: line.isWrapped };
    },
  };
}

const promptMarks = new Map<string, PromptMarks>();
/**
 * Prompt jumps and output selection on `paneId`'s terminal (task 8.45): `previous` and `next`
 * scroll to a prompt, `output` selects a finished command's output. False when nothing moved.
 */
export function promptAction(paneId: string, how: "previous" | "next" | "output"): boolean {
  const m = promptMarks.get(paneId);
  if (!m) return false;
  return how === "output" ? m.selectOutput() : m.jump(how === "previous");
}

const clearers = new Map<string, (keepScreen: boolean) => void>();
/**
 * Clears the terminal on `paneId` (task 8.44): `keepScreen` drops the history only (ED3), else
 * the prompt line becomes the first line and the history goes too. The daemon clears its copy,
 * so a reattach shows the same; an older daemon keeps its lines. False when no terminal is there.
 */
export function clearTerminal(paneId: string, keepScreen: boolean): boolean {
  const clear = clearers.get(paneId);
  clear?.(keepScreen);
  return !!clear;
}

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
export function TerminalPane({
  paneId,
  visible,
  focused,
  onFocus,
  open,
  openUrl,
  openPath,
  onTerminal,
  onSelection,
  tint,
  font = terminalFont,
  behaviour = terminalBehaviour,
  scheme,
  opacity,
  onBell,
  confirmPaste,
  onLinkClick,
  onLinkMenu,
}: TerminalPaneProps) {
  const live = useRef<Live | null>(null);
  const [ready, setReady] = useState(false);
  const callbacks = useRef({ open, openUrl, openPath, onTerminal, onFocus, onSelection, onBell, confirmPaste, onLinkClick, onLinkMenu });
  callbacks.current = { open, openUrl, openPath, onTerminal, onFocus, onSelection, onBell, confirmPaste, onLinkClick, onLinkMenu };
  const behaviourNow = useRef(behaviour);
  behaviourNow.current = behaviour;
  const behaviourKey = JSON.stringify(behaviour);
  const tintNow = useRef(tint);
  tintNow.current = tint;
  const lookNow = useRef({ scheme, opacity });
  lookNow.current = { scheme, opacity };
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
    // Copy on select writes from mouseup, a user gesture the clipboard accepts.
    const onMouseUp = () => {
      const term = live.current?.term;
      if (!behaviourNow.current.copyOnSelect || !term?.hasSelection()) return;
      void navigator.clipboard?.writeText(term.getSelection()).catch(() => undefined);
    };
    // Ahead of xterm.js's own listener: a risky paste waits for the person (task 8.42).
    const onPaste = (e: ClipboardEvent) => {
      const term = live.current?.term;
      const text = e.clipboardData?.getData("text/plain") ?? "";
      const ask = callbacks.current.confirmPaste;
      const reason = behaviourNow.current.pasteWarning && ask ? pasteRisk(text) : undefined;
      if (!term || !reason || !ask) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      void ask(text, reason).then((ok) => {
        if (ok && live.current?.term === term) term.paste(text);
        if (live.current?.term === term) term.focus();
      });
    };
    el.addEventListener("mouseup", onMouseUp);
    el.addEventListener("paste", onPaste, true);
    // A right click on a link opens the app's menu for it (task 8.49), not the word selection.
    let hoveredLink: TerminalLink | undefined;
    const onContextMenu = (e: MouseEvent) => {
      if (!hoveredLink || !callbacks.current.onLinkMenu) return;
      e.preventDefault();
      e.stopPropagation();
      callbacks.current.onLinkMenu(hoveredLink, { x: e.clientX, y: e.clientY });
    };
    el.addEventListener("contextmenu", onContextMenu, true);
    let disposed = false;
    let observer: ResizeObserver | undefined;
    let frame = 0;

    void (async () => {
      const startFont = fontNow.current;
      await fontsReady(startFont);
      if (disposed) return;
      // `⌘`-click opens (D27); a plain click copies; the hovered link feeds the right click.
      const clicked = (e: MouseEvent, link: TerminalLink) => callbacks.current.onLinkClick?.(link, { x: e.clientX, y: e.clientY });
      const activate = (e: MouseEvent, uri: string) => {
        if (!isHttp(uri)) return;
        if (e.metaKey) callbacks.current.openUrl(uri);
        else clicked(e, { kind: "url", text: uri });
      };
      const hover = (link: TerminalLink) => ({
        hover: () => (hoveredLink = link),
        leave: () => {
          if (hoveredLink === link) hoveredLink = undefined;
        },
      });
      const term = new Terminal({
        ...startFont,
        theme: themeFor(tintNow.current, lookNow.current.scheme, lookNow.current.opacity),
        allowTransparency: (lookNow.current.opacity ?? 100) < 100,
        // As Terminal.app's Basic profile: bold without bright colours. The behaviour defaults
        // give a blinking bar and Option as Meta, so Option+Enter sends Esc+Return.
        ...terminalBehaviourOptions(behaviourNow.current),
        drawBoldTextInBrightColors: false,
        linkHandler: { activate },
        // Search highlights and markers are decorations, still a proposed API in xterm.js 6.
        allowProposedApi: true,
        // The scrollbar's ruler shows search matches and prompt marks; it needs a width to draw.
        overviewRuler: { width: 10 },
      });
      const fit = new FitAddon();
      term.loadAddon(fit);
      const search = new SearchAddon({ highlightLimit: 2000 });
      term.loadAddon(search);
      // The daemon's grid measures with a newer table; Unicode 11 is the closest xterm.js has.
      term.loadAddon(new Unicode11Addon());
      term.unicode.activeVersion = "11";
      term.loadAddon(
        new WebLinksAddon(activate, {
          hover: (_e, text) => (hoveredLink = { kind: "url", text }),
          leave: () => (hoveredLink = undefined),
        }),
      );
      // File paths underline on hover like links and open on `⌘`-click (D27).
      term.registerLinkProvider({
        provideLinks(y, done) {
          const line = term.buffer.active.getLine(y - 1);
          if (!line || !callbacks.current.openPath) return done(undefined);
          const text = line.translateToString(true);
          // An address is the email provider's link, though it reads as a file name too.
          const mails = findEmails(text);
          const paths = findPaths(text).filter((p) => !mails.some((m) => p.start < m.end && m.start < p.end));
          const links = paths.map((p) => ({
            range: { start: { x: cellOf(line, p.start) + 1, y }, end: { x: cellOf(line, p.end - 1) + 1, y } },
            text: p.path,
            activate: (e: MouseEvent, path: string) => {
              if (e.metaKey) callbacks.current.openPath?.(path, e.altKey);
              else clicked(e, { kind: "path", text: path });
            },
            ...hover({ kind: "path", text: p.path }),
          }));
          done(links.length ? links : undefined);
        },
      });
      term.registerLinkProvider({
        provideLinks(y, done) {
          const line = term.buffer.active.getLine(y - 1);
          if (!line) return done(undefined);
          const links = findEmails(line.translateToString(true)).map((m) => ({
            range: { start: { x: cellOf(line, m.start) + 1, y }, end: { x: cellOf(line, m.end - 1) + 1, y } },
            text: m.text,
            activate: (e: MouseEvent, text: string) => {
              if (e.metaKey) callbacks.current.openUrl(`mailto:${text}`);
              else clicked(e, { kind: "email", text });
            },
            ...hover({ kind: "email", text: m.text }),
          }));
          done(links.length ? links : undefined);
        },
      });
      // ⌘ chords belong to the app menu (D17), including copy and paste. With one Option key as
      // Meta, pressing an Option key picks the mode before the key it modifies arrives.
      term.attachCustomKeyEventHandler((e) => {
        const meta = behaviourNow.current.optionAsMeta;
        if (e.type === "keydown" && e.key === "Alt" && (meta === "left" || meta === "right")) {
          term.options.macOptionIsMeta = e.code === (meta === "left" ? "AltLeft" : "AltRight");
        }
        return !e.metaKey;
      });
      term.onBell(() => {
        const kind = behaviourNow.current.bell;
        if (kind === "off") return;
        if (kind === "visual") {
          el.classList.remove("bell");
          void el.offsetWidth;
          el.classList.add("bell");
        }
        callbacks.current.onBell?.(kind);
      });
      term.open(el);
      el.dataset.renderer = "dom";
      promptMarks.set(paneId, attachPromptMarks(term, { tick: colors.dim, failed: colors.red }));
      buffers.set(paneId, term);
      const l: Live = { term, fit, search, early: 0, glLost: false };
      live.current = l;
      // addon-search 0.16.0 stores the new options before it compares them, so a changed toggle
      // would keep the old highlights: clearing drops its cached term and forces a new pass.
      let lastOpts = "";
      finders.set(paneId, {
        find: (query, opts, back, incremental) => {
          if (!query) {
            search.clearDecorations();
            lastOpts = "";
            return false;
          }
          const key = JSON.stringify(opts);
          if (key !== lastOpts) search.clearDecorations();
          lastOpts = key;
          const o: ISearchOptions = { ...opts, incremental, decorations: MATCHES };
          return back ? search.findPrevious(query, o) : search.findNext(query, o);
        },
        clear: () => {
          search.clearDecorations();
          lastOpts = "";
          term.clearSelection();
        },
        listen: (cb) => {
          const d = search.onDidChangeResults((r) => cb({ index: r.resultIndex, count: r.resultCount }));
          return () => d.dispose();
        },
      });
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
      clearers.set(paneId, (keepScreen) => {
        if (keepScreen) term.write("\x1b[3J");
        else term.clear();
        view.clear(keepScreen);
      });
      setReady(true);
    })();

    return () => {
      if (selections.get(paneId) === selected) selections.delete(paneId);
      finders.delete(paneId);
      clearers.delete(paneId);
      promptMarks.get(paneId)?.dispose();
      promptMarks.delete(paneId);
      if (buffers.get(paneId) === live.current?.term) buffers.delete(paneId);
      disposed = true;
      cancelAnimationFrame(frame);
      observer?.disconnect();
      el.removeEventListener("focusin", focusIn);
      el.removeEventListener(PIN_EVENT, onPin);
      el.removeEventListener(EXTRACT_EVENT, onExtract);
      el.removeEventListener(INSERT_EVENT, onInsert);
      el.removeEventListener(REVEAL_ROWS_EVENT, onReveal);
      el.removeEventListener("mouseup", onMouseUp);
      el.removeEventListener("paste", onPaste, true);
      el.removeEventListener("contextmenu", onContextMenu, true);
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
    if (!term) return;
    term.options.allowTransparency = (opacity ?? 100) < 100;
    term.options.theme = themeFor(tint, scheme, opacity);
  }, [paneId, tint, scheme, opacity, ready]);

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

  // Settings › Terminal's behaviour applies at once too; a shorter history drops the oldest lines.
  useEffect(() => {
    const term = live.current?.term;
    if (term) Object.assign(term.options, terminalBehaviourOptions(behaviourNow.current));
  }, [paneId, behaviourKey, ready]);

  return null;
}

import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import {
  bracketMatching,
  defaultHighlightStyle,
  indentOnInput,
  LanguageDescription,
  syntaxHighlighting,
} from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { highlightSelectionMatches, search, searchKeymap, selectNextOccurrence } from "@codemirror/search";
import { Annotation, Compartment, EditorState, type Extension } from "@codemirror/state";
import {
  crosshairCursor,
  drawSelection,
  dropCursor,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  rectangularSelection,
} from "@codemirror/view";
import type { DocView, Host } from "@terminescu/host";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { INSERT_EVENT, type InsertDetail } from "./events";
import { claimPane, paneElement, releasePane } from "./pane-registry";
import { eolOf, exactConfig, fitBreaks, textOf, type Eol } from "./text";
import { refreshChips, varChips, type VarInfo } from "./varchips";
import { useChord } from "./chords";

/** How long typing pauses before the text goes to the core (D28). */
export const SYNC_MS = 300;

/** What an editor starts from: the document's text and the pane's extensions. */
export interface EditorConfig {
  doc: string;
  extensions: Extension;
}

/** An editor in the pane: the view the pane syncs, and how it goes away. */
export interface MountedEditor {
  view: EditorView;
  destroy(): void;
  /** Takes new text from the core; without it the pane mounts a fresh editor. */
  reset?(config: EditorConfig): void;
}

/** Builds the pane's editor (Strategy): a plain one, or a diff tab's merge view (add-git-plugin D6). */
export type MountEditor = (parent: HTMLElement, config: EditorConfig) => MountedEditor;

export const plainEditor: MountEditor = (parent, config) => {
  const view = new EditorView({ parent, state: EditorState.create(config) });
  return { view, destroy: () => view.destroy(), reset: (c) => view.setState(EditorState.create(c)) };
};

export type TextHost = Pick<Host, "document" | "editDocument" | "onDocument" | "reloadDocument" | "keepMine" | "pasteImage">;

export interface TextPaneProps {
  paneId: string;
  docId: string;
  visible: boolean;
  focused: boolean;
  onFocus?(): void;
  host: TextHost;
  /** The document's title, dirty flag and bars changed. */
  onDoc?(doc: DocView): void;
  /** The side tab's variables, names and modes only, for `{{variable}}` chips (D29); absent
   * keeps the last list, as for a side tab not on screen. */
  variables?: readonly VarInfo[];
  /** Builds the editor; read when the pane mounts. */
  mount?: MountEditor;
  /** A bar above the editor, as a diff tab's. */
  bar?: ReactNode;
}

/** What the app asks of a text pane: send pending text before a save, read the selection. */
export interface TextHandle {
  flush(): Promise<void>;
  /** The selected text, or the whole document when nothing is selected. */
  selectionOrAll(): string;
  docId: string;
}

const handles = new Map<string, TextHandle>();

/** The text pane on `paneId`, if that pane is one. */
export function textPane(paneId: string): TextHandle | undefined {
  return handles.get(paneId);
}

/** Marks text the core sent, so it does not go back as an edit. */
const fromCore = Annotation.define<boolean>();

/** The text pane's look, for an editor beside it such as a diff's base side. */
export const editorTheme = EditorView.theme(
  {
    "&": { height: "100%", color: "var(--fg)", backgroundColor: "var(--panel)", fontSize: "13px" },
    ".cm-scroller": { fontFamily: '"Geist Mono", ui-monospace, monospace', lineHeight: "1.5" },
    ".cm-content": { caretColor: "var(--amber)" },
    ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--amber)", borderLeftWidth: "2px" },
    ".cm-gutters": { backgroundColor: "var(--panel)", color: "var(--faint)", border: "none" },
    ".cm-activeLine, .cm-activeLineGutter": { backgroundColor: "var(--panel-2)" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
      backgroundColor: "color-mix(in srgb, var(--amber) 30%, transparent)",
    },
    ".cm-panels": { backgroundColor: "var(--bg)", color: "var(--fg)" },
    ".cm-searchMatch": { backgroundColor: "color-mix(in srgb, var(--amber) 25%, transparent)" },
    ".cm-var-chip": {
      backgroundColor: "color-mix(in srgb, var(--amber) 18%, transparent)",
      border: "1px solid color-mix(in srgb, var(--amber) 45%, transparent)",
      borderRadius: "4px",
    },
    ".cm-var-secret": { borderStyle: "dashed" },
    ".cm-var-unknown": { textDecoration: "underline wavy var(--red)", textUnderlineOffset: "3px" },
  },
  { dark: true },
);

/** The language for a document: by file name, Markdown for buffers, prompts and variables. */
function languageFor(doc: DocView): LanguageDescription | "markdown" | undefined {
  if (doc.source.kind !== "file") return "markdown";
  const found = LanguageDescription.matchFilename(languages, doc.title);
  if (!found || found.name === "Markdown") return "markdown";
  return found;
}

// Chords that belong to the app menu (D17) are not bound here: ⌘D splits, so the next
// occurrence takes ⌃⌘G, and ⌘↵ returns an editor request.
const searchKeys = searchKeymap.filter((k) => k.key !== "Mod-d");
const editKeys = defaultKeymap.filter((k) => k.key !== "Mod-Enter");

function baseExtensions(language: Compartment, readOnly: Compartment, onUpdate: Extension, extra: Extension): Extension {
  return [
    extra,
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightSpecialChars(),
    history(),
    drawSelection(),
    dropCursor(),
    EditorState.allowMultipleSelections.of(true),
    indentOnInput(),
    syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
    bracketMatching(),
    closeBrackets(),
    rectangularSelection(),
    crosshairCursor(),
    highlightActiveLine(),
    highlightSelectionMatches(),
    search({ top: true }),
    EditorView.lineWrapping,
    keymap.of([
      { key: "Ctrl-Mod-g", run: selectNextOccurrence, preventDefault: true },
      ...closeBracketsKeymap,
      ...editKeys,
      ...searchKeys,
      ...historyKeymap,
      indentWithTab,
    ]),
    editorTheme,
    language.of([]),
    readOnly.of(EditorState.readOnly.of(false)),
    onUpdate,
  ];
}

interface Cursor {
  line: number;
  col: number;
  cursors: number;
}

/**
 * A CodeMirror 6 editor on one document (D27, D29). Like the terminal, it lives in the pane's
 * element from the registry, so a split or a tab move keeps its state. Typing reaches the core
 * after a pause; the core sends reloads and the changed-on-disk state back.
 */
export function TextPane({ paneId, docId, visible, focused, onFocus, host, onDoc, variables, mount = plainEditor, bar }: TextPaneProps) {
  const saveChord = useChord("save");
  const holder = useRef<HTMLDivElement>(null);
  const editor = useRef<EditorView | null>(null);
  const [doc, setDoc] = useState<DocView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [cursor, setCursor] = useState<Cursor>({ line: 1, col: 1, cursors: 1 });
  const [eol, setEol] = useState<Eol>("LF");
  const [langName, setLangName] = useState("Plain text");
  const callbacks = useRef({ host, onDoc, onFocus });
  callbacks.current = { host, onDoc, onFocus };
  // What the chips read: on for prompts, buffers, variables and Markdown files.
  const chips = useRef<{ on: boolean; vars: readonly VarInfo[] }>({ on: false, vars: variables ?? [] });
  // Synced with the core: the last document seen and the edits queued in order.
  // `inflight` counts edits sent and not answered yet.
  const sync = useRef({ rev: -1, timer: 0, pending: false, inflight: 0, chain: Promise.resolve() as Promise<void> });
  // Puts text from the core into the editor; set while the editor lives.
  const loader = useRef<((d: DocView) => void) | null>(null);

  const shown = (d: DocView) => {
    // A reply carries the core's revision; a skipped reload already has a stale edit queued.
    sync.current.rev = d.rev;
    const { text: _text, ...rest } = d;
    setDoc(rest);
    callbacks.current.onDoc?.(rest);
  };

  useEffect(() => {
    const el = claimPane(paneId);
    el.classList.add("text-host");
    const focusIn = () => callbacks.current.onFocus?.();
    el.addEventListener("focusin", focusIn);
    const language = new Compartment();
    const readOnly = new Compartment();
    let disposed = false;
    const s = sync.current;
    const pasteImage = (file: File, view: EditorView) => {
      const ext = file.type.slice("image/".length).replace("jpeg", "jpg");
      void file
        .arrayBuffer()
        .then((b) => callbacks.current.host.pasteImage(docId, new Uint8Array(b), ext))
        .then(
          (name) => {
            if (disposed || view.state.readOnly) return;
            const link = chips.current.on ? `![](${encodeURI(name)})` : name;
            view.dispatch(view.state.replaceSelection(link));
          },
          (e: unknown) => !disposed && setError(String(e)),
        );
    };
    const extra: Extension = [
      EditorView.clipboardInputFilter.of(fitBreaks),
      varChips(() => chips.current),
      EditorView.domEventHandlers({
        paste(e, view) {
          const file = [...(e.clipboardData?.files ?? [])].find((f) => /^image\/(png|jpeg|gif|webp)$/.test(f.type));
          if (!file || view.state.readOnly) return false;
          e.preventDefault();
          pasteImage(file, view);
          return true;
        },
      }),
    ];

    const send = (): Promise<void> => {
      clearTimeout(s.timer);
      const view = editor.current;
      if (!s.pending || !view) return s.chain;
      s.pending = false;
      const text = textOf(view.state);
      const base = s.rev;
      s.inflight++;
      s.chain = s.chain
        .then(() => callbacks.current.host.editDocument(docId, text, base))
        .then((d) => {
          if (!disposed) shown(d);
        })
        .catch((e: unknown) => {
          if (!disposed) setError(String(e));
        })
        .finally(() => {
          s.inflight--;
        });
      return s.chain;
    };

    const onUpdate = EditorView.updateListener.of((u) => {
      if (u.selectionSet || u.docChanged) {
        const sel = u.state.selection;
        const head = sel.main.head;
        const line = u.state.doc.lineAt(head);
        setCursor({ line: line.number, col: head - line.from + 1, cursors: sel.ranges.length });
      }
      if (!u.docChanged || u.transactions.some((t) => t.annotation(fromCore))) return;
      s.pending = true;
      clearTimeout(s.timer);
      s.timer = window.setTimeout(() => void send(), SYNC_MS);
    });

    // Set once the editor below is mounted; `load` runs only after that.
    let mounted: MountedEditor | undefined;
    const load = (d: DocView) => {
      if (!mounted || !editor.current || d.text === undefined) return;
      s.rev = d.rev;
      s.pending = false;
      clearTimeout(s.timer);
      const keep = Math.min(editor.current.state.selection.main.head, d.text.length);
      const config = exactConfig(d.text, baseExtensions(language, readOnly, onUpdate, extra));
      if (mounted.reset) mounted.reset(config);
      else {
        // A merge view keeps extensions of its own: it is built again around the new text.
        mounted.destroy();
        mounted = mount(holder.current!, config);
        editor.current = mounted.view;
      }
      const view = mounted.view;
      view.dispatch({
        selection: { anchor: Math.min(keep, view.state.doc.length) },
        effects: readOnly.reconfigure(EditorState.readOnly.of(d.readOnly)),
        annotations: fromCore.of(true),
      });
      setEol(eolOf(d.text));
      const lang = languageFor(d);
      chips.current.on = lang === "markdown";
      if (lang === "markdown") {
        setLangName("Markdown");
        view.dispatch({ effects: language.reconfigure(markdown({ codeLanguages: languages })) });
      } else if (lang) {
        setLangName(lang.name);
        void lang.load().then(
          (support) => !disposed && editor.current?.dispatch({ effects: language.reconfigure(support) }),
          () => undefined,
        );
      } else setLangName("Plain text");
      shown(d);
    };

    loader.current = load;
    mounted = mount(holder.current!, exactConfig("", baseExtensions(language, readOnly, onUpdate, extra)));
    editor.current = mounted.view;
    const handle: TextHandle = {
      docId,
      flush: send,
      selectionOrAll() {
        const state = editor.current?.state;
        if (!state) return "";
        const parts = state.selection.ranges.filter((r) => !r.empty).map((r) => state.sliceDoc(r.from, r.to));
        return parts.length ? parts.join(state.lineBreak) : textOf(state);
      },
    };
    handles.set(paneId, handle);

    // A prompt or pin inserted from the context panel lands at the cursor.
    const onInsert = (e: Event) => {
      const view = editor.current;
      const d = (e as CustomEvent<InsertDetail>).detail;
      if (!view || !d?.text || view.state.readOnly) return;
      view.dispatch(view.state.replaceSelection(fitBreaks(d.text, view.state)));
      view.focus();
    };
    el.addEventListener(INSERT_EVENT, onInsert);

    let unlisten: (() => void) | undefined;
    void (async () => {
      const off = await callbacks.current.host.onDocument((d) => {
        if (d.id !== docId || disposed) return;
        if (d.text !== undefined && d.rev !== s.rev) {
          // Typing the core has not seen yet stays: the edit goes over the old revision and
          // comes back as a conflict, which shows Reload and Keep mine (D28).
          if (s.pending || s.inflight > 0) {
            if (s.pending) void send();
            return;
          }
          load(d);
        } else shown(d);
      });
      if (disposed) return off();
      unlisten = off;
      try {
        const d = await callbacks.current.host.document(docId);
        if (!disposed) load(d);
      } catch (e) {
        if (!disposed) setError(String(e));
      }
    })();

    return () => {
      disposed = true;
      // Text typed in the last moments still goes to the core.
      void send();
      unlisten?.();
      if (handles.get(paneId) === handle) handles.delete(paneId);
      el.removeEventListener("focusin", focusIn);
      el.removeEventListener(INSERT_EVENT, onInsert);
      mounted?.destroy();
      editor.current = null;
      loader.current = null;
      el.classList.remove("text-host");
      releasePane(paneId);
    };
  }, [paneId, docId]);

  // New variables redraw the chips; names and modes only, so a value change does nothing.
  const varsKey = variables?.map((v) => `${v.name}:${v.mode}`).join(",");
  useEffect(() => {
    if (!variables) return;
    chips.current.vars = variables;
    if (editor.current) refreshChips(editor.current);
  }, [varsKey]);

  useEffect(() => {
    paneElement(paneId).hidden = !visible;
    if (visible) editor.current?.requestMeasure();
  }, [paneId, visible]);

  useEffect(() => {
    if (visible && focused) editor.current?.focus();
  }, [visible, focused, doc === null]);

  // Reload brings the disk's text; Keep mine only clears the bar.
  const act = (f: (id: string) => Promise<DocView>) => () =>
    void f(docId).then(
      (d) => (d.text !== undefined && loader.current ? loader.current(d) : shown(d)),
      (e: unknown) => setError(String(e)),
    );

  return createPortal(
    <div className="text-pane" data-doc={docId}>
      {doc?.conflict && (
        <div className="text-bar text-bar-warn" role="alert">
          <span>{doc.title} changed on disk.</span>
          <button type="button" className="btn" onClick={act((id) => callbacks.current.host.reloadDocument(id))}>
            Reload
          </button>
          <button type="button" className="btn" onClick={act((id) => callbacks.current.host.keepMine(id))}>
            Keep mine
          </button>
        </div>
      )}
      {bar}
      {doc?.note && <div className="text-bar text-bar-warn">{doc.note}</div>}
      {doc?.missing && !doc.conflict && <div className="text-bar">Not on disk yet. {saveChord ? `${saveChord} creates it.` : "Save creates it."}</div>}
      {error && (
        <div className="text-bar text-bar-error" role="alert">
          {error}
        </div>
      )}
      <div ref={holder} className="text-editor" />
      <div className="text-status">
        <span>
          Ln {cursor.line}, Col {cursor.col}
        </span>
        {cursor.cursors > 1 && <span>{cursor.cursors} cursors</span>}
        <span>{eol}</span>
        <span>{doc?.bom ? "UTF-8 BOM" : "UTF-8"}</span>
        <span>{langName}</span>
        {doc?.readOnly && <span>Read-only</span>}
        {doc?.sessionOnly && <span>Session only</span>}
      </div>
    </div>,
    paneElement(paneId),
  );
}

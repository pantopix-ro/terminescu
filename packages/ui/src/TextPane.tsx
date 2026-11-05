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
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { INSERT_EVENT, type InsertDetail } from "./events";
import { claimPane, paneElement, releasePane } from "./pane-registry";
import { eolOf, exactState, textOf, type Eol } from "./text";

/** How long typing pauses before the text goes to the core (D28). */
export const SYNC_MS = 300;

export type TextHost = Pick<Host, "document" | "editDocument" | "onDocument" | "reloadDocument" | "keepMine">;

export interface TextPaneProps {
  paneId: string;
  docId: string;
  visible: boolean;
  focused: boolean;
  onFocus?(): void;
  host: TextHost;
  /** The document's title, dirty flag and bars changed. */
  onDoc?(doc: DocView): void;
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

const theme = EditorView.theme(
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

function baseExtensions(language: Compartment, readOnly: Compartment, onUpdate: Extension): Extension {
  return [
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
    theme,
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
export function TextPane({ paneId, docId, visible, focused, onFocus, host, onDoc }: TextPaneProps) {
  const holder = useRef<HTMLDivElement>(null);
  const editor = useRef<EditorView | null>(null);
  const [doc, setDoc] = useState<DocView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [cursor, setCursor] = useState<Cursor>({ line: 1, col: 1, cursors: 1 });
  const [eol, setEol] = useState<Eol>("LF");
  const [langName, setLangName] = useState("Plain text");
  const callbacks = useRef({ host, onDoc, onFocus });
  callbacks.current = { host, onDoc, onFocus };
  // Synced with the core: the last document seen and the edits queued in order.
  const sync = useRef({ rev: -1, timer: 0, pending: false, chain: Promise.resolve() as Promise<void> });
  // Puts text from the core into the editor; set while the editor lives.
  const loader = useRef<((d: DocView) => void) | null>(null);

  const shown = (d: DocView) => {
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

    const send = (): Promise<void> => {
      clearTimeout(s.timer);
      const view = editor.current;
      if (!s.pending || !view) return s.chain;
      s.pending = false;
      const text = textOf(view.state);
      s.chain = s.chain
        .then(() => callbacks.current.host.editDocument(docId, text))
        .then((d) => {
          if (!disposed) shown(d);
        })
        .catch((e: unknown) => {
          if (!disposed) setError(String(e));
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

    const load = (d: DocView) => {
      const view = editor.current;
      if (!view || d.text === undefined) return;
      s.rev = d.rev;
      s.pending = false;
      clearTimeout(s.timer);
      const keep = Math.min(view.state.selection.main.head, d.text.length);
      view.setState(exactState(d.text, baseExtensions(language, readOnly, onUpdate)));
      view.dispatch({
        selection: { anchor: Math.min(keep, view.state.doc.length) },
        effects: readOnly.reconfigure(EditorState.readOnly.of(d.readOnly)),
        annotations: fromCore.of(true),
      });
      setEol(eolOf(d.text));
      const lang = languageFor(d);
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
    editor.current = new EditorView({
      parent: holder.current!,
      state: exactState("", baseExtensions(language, readOnly, onUpdate)),
    });
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
      view.dispatch(view.state.replaceSelection(d.text));
      view.focus();
    };
    el.addEventListener(INSERT_EVENT, onInsert);

    let unlisten: (() => void) | undefined;
    void (async () => {
      const off = await callbacks.current.host.onDocument((d) => {
        if (d.id !== docId || disposed) return;
        if (d.text !== undefined && d.rev !== s.rev) load(d);
        else shown(d);
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
      editor.current?.destroy();
      editor.current = null;
      loader.current = null;
      el.classList.remove("text-host");
      releasePane(paneId);
    };
  }, [paneId, docId]);

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
      {doc?.note && <div className="text-bar text-bar-warn">{doc.note}</div>}
      {doc?.missing && !doc.conflict && <div className="text-bar">Not on disk yet. ⌘S creates it.</div>}
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

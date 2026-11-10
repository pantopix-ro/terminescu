// A diff tab's editors (add-git-plugin D6): the text pane's own editor, inside a merge view.
// The working side keeps every extension the pane gives it, so saving and line endings work
// as in a text tab. Each mount registers a live handle (polish-git-plugin D4), so a new base
// goes into the open editor and a layout switch can put the scroll back.

import { MergeView, unifiedMergeView, updateOriginalDoc } from "@codemirror/merge";
import { ChangeSet, EditorState, Text, Transaction, type Extension } from "@codemirror/state";
import { EditorView, lineNumbers } from "@codemirror/view";
import { editorTheme, fromCore, type MountEditor } from "./TextPane";
import { separatorOf } from "./text";

/** The diff colours, given to every editor of a diff tab so they win over the merge view's own. */
export const diffTheme = EditorView.theme({
  ".cm-changedLine": { backgroundColor: "color-mix(in srgb, var(--amber) 10%, transparent)" },
  ".cm-changedText": { background: "color-mix(in srgb, var(--amber) 28%, transparent)" },
  ".cm-deletedChunk": { backgroundColor: "color-mix(in srgb, var(--red) 12%, transparent)" },
  ".cm-insertedLine": { backgroundColor: "color-mix(in srgb, var(--green) 10%, transparent)" },
  ".cm-deletedLine, .cm-deletedText": { backgroundColor: "color-mix(in srgb, var(--red) 18%, transparent)" },
});

/** The open diff editor, as its mount registers it. DiffPane keeps one per tab. */
export interface DiffLive {
  /** Puts a new base text into the open editor. */
  setBase?(text: string): void;
  /** The element that scrolls. */
  scroller?(): HTMLElement | undefined;
  /** A scroll top to put back once the next mount is in place. */
  restore?: number;
}

/** The change from `a` to `b` as one replace of the middle both ends do not share. */
export function middleChange(a: string, b: string): { from: number; to: number; insert: string } | undefined {
  if (a === b) return undefined;
  let head = 0;
  const max = Math.min(a.length, b.length);
  while (head < max && a.charCodeAt(head) === b.charCodeAt(head)) head++;
  let tail = 0;
  while (tail < max - head && a.charCodeAt(a.length - 1 - tail) === b.charCodeAt(b.length - 1 - tail)) tail++;
  return { from: head, to: a.length - tail, insert: b.slice(head, b.length - tail) };
}

/** Writes `text` into `view` as one quiet change: no undo step, and the pane does not send it. */
function replaceQuietly(view: EditorView, text: string, fromCoreText: boolean) {
  const change = middleChange(view.state.doc.toString(), text);
  if (!change) return;
  view.dispatch({
    changes: change,
    annotations: [Transaction.addToHistory.of(false), ...(fromCoreText ? [fromCore.of(true)] : [])],
  });
}

/** The effect that moves a unified view's base from `base` to `text`; none when they match. */
export function baseUpdate(base: string, text: string) {
  const change = middleChange(base, text);
  if (!change) return undefined;
  return updateOriginalDoc.of({ doc: Text.of(text.split("\n")), changes: ChangeSet.of(change, base.length) });
}

/** The editor splits lines as `text` needs; a text with other line breaks needs a new editor. */
const sameBreaks = (view: EditorView, text: string) => view.state.facet(EditorState.lineSeparator) === separatorOf(text);

const locked: Extension = [EditorState.readOnly.of(true), EditorView.editable.of(false)];

/** Puts the scroll back after a mount, once the editor has its height. */
function restoreScroll(live: DiffLive | undefined) {
  const top = live?.restore;
  if (live) delete live.restore;
  if (top === undefined) return;
  requestAnimationFrame(() => {
    const el = live?.scroller?.();
    if (el) el.scrollTop = top;
  });
}

/** Side by side: the base on the left, read-only; the working copy on the right. */
export function sideBySide(original: string, live?: DiffLive): MountEditor {
  // The base as the left editor shows it now, for an editor built again.
  let base = original;
  return (parent, config) => {
    const merge = new MergeView({
      parent,
      a: { doc: base, extensions: [lineNumbers(), editorTheme, diffTheme, EditorView.lineWrapping, locked] },
      b: { doc: config.doc, extensions: [config.extensions, diffTheme] },
      gutter: true,
      highlightChanges: true,
    });
    if (live) {
      live.setBase = (text) => {
        base = text;
        replaceQuietly(merge.a, text, false);
      };
      live.scroller = () => merge.dom;
      restoreScroll(live);
    }
    return {
      view: merge.b,
      destroy: () => merge.destroy(),
      // New text from the core changes only what differs, so the place stays. Other line
      // breaks need the editor built again.
      reset: (c) => sameBreaks(merge.b, c.doc) && (replaceQuietly(merge.b, c.doc, true), true),
    };
  };
}

/** Unified: one editor, the removed lines shown above the lines that replaced them. */
export function unified(original: string, live?: DiffLive): MountEditor {
  let base = original;
  const withDiff = (extensions: Extension): Extension => [
    extensions,
    diffTheme,
    unifiedMergeView({ original: base, mergeControls: false, gutter: true }),
  ];
  return (parent, config) => {
    const view = new EditorView({ parent, state: EditorState.create({ doc: config.doc, extensions: withDiff(config.extensions) }) });
    if (live) {
      live.setBase = (text) => {
        const effect = baseUpdate(base, text);
        base = text;
        if (effect) view.dispatch({ effects: effect });
      };
      live.scroller = () => view.scrollDOM;
      restoreScroll(live);
    }
    return {
      view,
      destroy: () => view.destroy(),
      reset: (c) => {
        if (sameBreaks(view, c.doc)) replaceQuietly(view, c.doc, true);
        else view.setState(EditorState.create({ doc: c.doc, extensions: withDiff(c.extensions) }));
      },
    };
  };
}

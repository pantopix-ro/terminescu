// A diff tab (add-git-plugin 3.2, add-claude-insights 3.2): the app's text pane over the working
// file, compared with a base text a plugin reads. The git plugin compares with HEAD, the Claude
// plugin with the file before the session; the layout is shared (diff-mode.ts). A new base goes
// into the open editor and a layout switch puts the scroll back (polish-git-plugin D4).

import { Fragment, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { sideBySide, unified, type DiffLive } from "./diff-merge";
import { loadDiffMode, onDiffMode, saveDiffMode, toggledDiffMode, type DiffMode } from "./diff-mode";
import { useChord, withChord } from "./chords";

/** A file's base text, as both plugins' `baseText` calls answer it. */
export interface DiffBase {
  exists: boolean;
  binary?: boolean;
  large?: boolean;
  size?: number;
  text?: string;
}

/** What the bar says while reading, over a file the base lacks, and over a plain change. */
export interface DiffNotes {
  reading: string;
  created: string;
  changed: string;
}

type Base = { answer: DiffBase } | { error: string };

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** What the bar says over a file gone from the working tree. */
export const DELETED_NOTE = "Deleted: the file is gone from the working tree.";

/** What the bar says about the comparison. */
export function diffNote(base: Base | undefined, notes: DiffNotes, deleted = false): string {
  if (!base) return notes.reading;
  if (deleted && !("error" in base)) return DELETED_NOTE;
  if ("error" in base) return `No diff: ${base.error}`;
  const a = base.answer;
  if (!a.exists) return notes.created;
  if (a.binary) return "Binary file: no diff.";
  if (a.large) return "Too large to compare.";
  return notes.changed;
}

export interface DiffPaneProps {
  /** Names what is compared; a new key reads the base again from scratch. */
  readKey: string;
  read(): Promise<DiffBase>;
  /** Signals that the base may have changed (a commit); returns the unsubscribe. */
  subscribe?(again: () => void): () => void;
  notes: DiffNotes;
  /** The plugin's `app.renderText`. */
  renderText?(options?: { mount?: unknown; bar?: ReactNode; readOnly?: boolean }): ReactNode;
  /** The file is gone from the working tree: the base shows read-only, with a note. */
  deleted?: boolean;
}

export function DiffPane({ readKey, read, subscribe, notes, renderText, deleted = false }: DiffPaneProps) {
  const chord = useChord("diff_layout");
  const [mode, setMode] = useState<DiffMode>(() => loadDiffMode());
  const [base, setBase] = useState<Base>();
  // Counts base texts that differ from the last, so the editor is built again around each.
  const [rev, setRev] = useState(0);
  // The latest callbacks, so a parent's new closures do not read the base again.
  const calls = useRef({ read, subscribe });
  calls.current = { read, subscribe };
  // The open editor's handle. A layout switch starts a new one that carries the scroll top, so
  // the editor built next scrolls back; the old editor's handle is never called again.
  const live = useRef<DiffLive>({});
  const shownMode = useRef(mode);
  if (shownMode.current !== mode) {
    shownMode.current = mode;
    live.current = { restore: live.current.scroller?.()?.scrollTop };
  }

  useEffect(() => onDiffMode(setMode), []);

  useEffect(() => {
    let gone = false;
    let last: string | undefined;
    // The base text the open editor shows, when it shows one.
    let shownText: string | undefined;
    // Only the newest read lands: an older base answering late must not replace a newer one.
    let asked = 0;
    const again = () => {
      const n = ++asked;
      void calls.current.read().then(
        (answer) => {
          if (gone || n !== asked) return;
          const text = answer.exists ? (answer.text ?? null) : "";
          const key = JSON.stringify([answer.exists, answer.binary, answer.large, text]);
          if (key === last) return;
          last = key;
          setBase({ answer });
          // Text after text goes into the open editor; anything else builds it again.
          const setInPlace = live.current.setBase;
          if (typeof text === "string" && shownText !== undefined && setInPlace) {
            shownText = text;
            setInPlace(text);
            return;
          }
          shownText = typeof text === "string" ? text : undefined;
          live.current = { restore: live.current.scroller?.()?.scrollTop };
          setRev((r) => r + 1);
        },
        (e: unknown) => {
          if (gone || n !== asked) return;
          // The next good answer shows again, even when it matches the one before the error.
          last = undefined;
          shownText = undefined;
          setBase({ error: message(e) });
        },
      );
    };
    again();
    const off = calls.current.subscribe?.(again);
    return () => {
      gone = true;
      off?.();
    };
  }, [readKey]);

  const original = base && "answer" in base ? (base.answer.exists ? base.answer.text : "") : undefined;
  // Built again only with the editor: a base that went in place leaves the editor as it is.
  const mount = useMemo(
    () => (original === undefined ? undefined : mode === "unified" ? unified(original, live.current) : sideBySide(original, live.current)),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [rev, mode, original === undefined],
  );
  if (!base || !renderText) return null;
  const bar = (
    <div className="text-bar diff-bar" data-mode={mode}>
      <span className="diff-note">{diffNote(base, notes, deleted)}</span>
      {mount && (
        <button type="button" className="btn diff-mode" title={withChord("Switch the diff layout", chord)} onClick={() => saveDiffMode(toggledDiffMode(mode))}>
          {mode === "side" ? "Unified" : "Side by side"}
        </button>
      )}
    </div>
  );
  // A new layout or a new kind of base builds the editor again; the text stays in the core.
  return <Fragment key={`${mode}:${rev}:${deleted}`}>{renderText({ mount, bar, readOnly: deleted })}</Fragment>;
}

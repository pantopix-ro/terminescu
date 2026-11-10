// A diff tab (add-git-plugin 3.2, add-claude-insights 3.2): the app's text pane over the working
// file, compared with a base text a plugin reads. The git plugin compares with HEAD, the Claude
// plugin with the file before the session; the layout is shared (diff-mode.ts).

import { Fragment, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { sideBySide, unified } from "./diff-merge";
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

/** What the bar says about the comparison. */
export function diffNote(base: Base | undefined, notes: DiffNotes): string {
  if (!base) return notes.reading;
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
  renderText?(options?: { mount?: unknown; bar?: ReactNode }): ReactNode;
}

export function DiffPane({ readKey, read, subscribe, notes, renderText }: DiffPaneProps) {
  const chord = useChord("diff_layout");
  const [mode, setMode] = useState<DiffMode>(() => loadDiffMode());
  const [base, setBase] = useState<Base>();
  // Counts base texts that differ from the last, so the editor is built again around each.
  const [rev, setRev] = useState(0);
  // The latest callbacks, so a parent's new closures do not read the base again.
  const calls = useRef({ read, subscribe });
  calls.current = { read, subscribe };

  useEffect(() => onDiffMode(setMode), []);

  useEffect(() => {
    let gone = false;
    let last: string | undefined;
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
          setRev((r) => r + 1);
        },
        (e: unknown) => {
          if (gone || n !== asked) return;
          // The next good answer shows again, even when it matches the one before the error.
          last = undefined;
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
  const mount = useMemo(() => (original === undefined ? undefined : mode === "unified" ? unified(original) : sideBySide(original)), [original, mode]);
  if (!base || !renderText) return null;
  const bar = (
    <div className="text-bar diff-bar" data-mode={mode}>
      <span className="diff-note">{diffNote(base, notes)}</span>
      {mount && (
        <button type="button" className="btn diff-mode" title={withChord("Switch the diff layout", chord)} onClick={() => saveDiffMode(toggledDiffMode(mode))}>
          {mode === "side" ? "Unified" : "Side by side"}
        </button>
      )}
    </div>
  );
  // A new layout or base builds the editor again; the text stays in the core.
  return <Fragment key={`${mode}:${rev}`}>{renderText({ mount, bar })}</Fragment>;
}

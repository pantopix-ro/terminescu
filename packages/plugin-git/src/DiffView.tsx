// The diff tab (add-git-plugin 3.2): the app's text pane over the working file, compared with
// the file at HEAD. It reads the base again when the Rust half reports new facts, as after a
// commit.

import { Fragment, useEffect, useMemo, useState } from "react";
import type { PaneViewProps, TextView } from "@terminescu/plugin-api";
import { sideBySide, unified } from "./merge";
import { loadMode, MODE_EVENT, saveMode, toggled, type DiffMode } from "./mode";

export const DIFF_KIND = "git-diff";

/** What a diff tab compares: a file of the working tree at `root`. */
export interface DiffSource {
  root: string;
  /** Relative to the root. */
  path: string;
}

export const diffView = (source: DiffSource): TextView => ({ kind: DIFF_KIND, source: JSON.stringify(source) });

export function parseSource(source: string): DiffSource | undefined {
  try {
    const s = JSON.parse(source) as Partial<DiffSource>;
    return typeof s.root === "string" && typeof s.path === "string" ? { root: s.root, path: s.path } : undefined;
  } catch {
    return undefined;
  }
}

/** The Rust half's `baseText` answer. */
export interface BaseText {
  exists: boolean;
  binary?: boolean;
  large?: boolean;
  size?: number;
  text?: string;
}

type Base = { answer: BaseText } | { error: string };

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** What the bar says about the comparison. */
export function baseNote(base: Base | undefined): string {
  if (!base) return "Reading HEAD…";
  if ("error" in base) return `No diff: ${base.error}`;
  const a = base.answer;
  if (!a.exists) return "New file: not in HEAD.";
  if (a.binary) return "Binary file: no diff.";
  if (a.large) return "Too large to compare.";
  return "Changes since HEAD.";
}

export function DiffView({ source, bridge, app }: PaneViewProps) {
  const src = useMemo(() => parseSource(source), [source]);
  const [mode, setMode] = useState<DiffMode>(() => loadMode());
  const [base, setBase] = useState<Base>();
  // Counts base texts that differ from the last, so the editor is built again around each.
  const [rev, setRev] = useState(0);

  useEffect(() => {
    const follow = (e: Event) => setMode((e as CustomEvent<DiffMode>).detail);
    window.addEventListener(MODE_EVENT, follow);
    return () => window.removeEventListener(MODE_EVENT, follow);
  }, []);

  useEffect(() => {
    if (!src) return void setBase({ error: "the tab names no file" });
    let gone = false;
    let last: string | undefined;
    // Only the newest read lands: an older HEAD answering late must not replace a newer one.
    let asked = 0;
    const read = () => {
      const n = ++asked;
      void bridge.call<BaseText>("baseText", { root: src.root, path: src.path }).then(
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
    read();
    const off = bridge.on("facts", read);
    return () => {
      gone = true;
      off();
    };
  }, [src, bridge]);

  const original = base && "answer" in base ? (base.answer.exists ? base.answer.text : "") : undefined;
  const mount = useMemo(() => (original === undefined ? undefined : mode === "unified" ? unified(original) : sideBySide(original)), [original, mode]);
  if (!base || !app.renderText) return null;
  const bar = (
    <div className="text-bar git-diff-bar" data-mode={mode}>
      <span className="git-diff-note">{baseNote(base)}</span>
      {mount && (
        <button type="button" className="btn git-diff-mode" title="⌥⌘U" onClick={() => saveMode(toggled(mode))}>
          {mode === "side" ? "Unified" : "Side by side"}
        </button>
      )}
    </div>
  );
  // A new layout or base builds the editor again; the text stays in the core.
  return <Fragment key={`${mode}:${rev}`}>{app.renderText({ mount, bar })}</Fragment>;
}

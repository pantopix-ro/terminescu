// "What Claude changed" (add-claude-insights 3.2): the files the shown tab's Claude session
// changed since it started, each opening as a diff against its text before the session. Read
// again when the Rust half reports a tool result or a finished turn.

import { useEffect, useMemo, useState } from "react";
import type { PaneViewProps, PanelSectionProps, TextView } from "@terminescu/plugin-api";
import { DiffPane, type DiffBase } from "@terminescu/ui";

/** One changed file, as the Rust half's `changes` call answers it. */
export interface ClaudeChange {
  /** Absolute. */
  path: string;
  added?: number | null;
  removed?: number | null;
  created: boolean;
  deleted: boolean;
  /** Why there is nothing to compare with, as "too large". */
  none?: string;
}

export interface ClaudeChanges {
  session: string;
  files: ClaudeChange[];
  /** A git snapshot covers the session, so Bash edits show too. */
  tree: boolean;
  /** The folder the paths show relative to. */
  base: string;
}

export const CLAUDE_DIFF = "claude-diff";

/** What a Claude diff tab compares: a file against its text before `session`. */
export interface ClaudeDiffSource {
  session: string;
  path: string;
}

export const claudeDiff = (source: ClaudeDiffSource): TextView => ({ kind: CLAUDE_DIFF, source: JSON.stringify(source) });

export function parseClaudeDiff(source: string): ClaudeDiffSource | undefined {
  try {
    const s = JSON.parse(source) as Partial<ClaudeDiffSource>;
    return typeof s.session === "string" && typeof s.path === "string" ? { session: s.session, path: s.path } : undefined;
  } catch {
    return undefined;
  }
}

/** The mark beside a file: `A` made in the session, `D` deleted, `M` changed. */
export function changeMark(c: ClaudeChange): { text: string; title: string } {
  if (c.created) return { text: "A", title: "Added in this session" };
  if (c.deleted) return { text: "D", title: "Deleted in this session" };
  return { text: "M", title: "Changed in this session" };
}

/** The path under `base` as "dir/" and "name"; a path outside it keeps its whole folder. */
export function splitUnder(path: string, base: string): { dir: string; name: string } {
  const root = base.replace(/\/+$/, "") + "/";
  const rel = path.startsWith(root) ? path.slice(root.length) : path;
  const at = rel.lastIndexOf("/");
  return at < 0 ? { dir: "", name: rel } : { dir: rel.slice(0, at + 1), name: rel.slice(at + 1) };
}

/** How long the list waits after a change event before it reads again. */
const RELOAD_MS = 300;

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function ChangesSection({ paneId, bridge, app }: PanelSectionProps) {
  const [state, setState] = useState<{ answer?: ClaudeChanges; error?: string }>({});
  const [openError, setOpenError] = useState<string>();
  useEffect(() => {
    setState({});
    if (!paneId) return;
    let gone = false;
    let asked = 0;
    const load = () => {
      const n = ++asked;
      const settle = (s: { answer?: ClaudeChanges; error?: string }) => !gone && n === asked && setState(s);
      bridge.call<ClaudeChanges>("changes", { pane: paneId }).then(
        (answer) => settle({ answer }),
        (e: unknown) => settle({ error: message(e) }),
      );
    };
    load();
    // A busy session edits many times a second; one read follows the last event of a burst.
    let timer: ReturnType<typeof setTimeout> | undefined;
    const mine = (p: unknown) => {
      if ((p as { pane?: string } | null)?.pane !== paneId) return;
      clearTimeout(timer);
      timer = setTimeout(load, RELOAD_MS);
    };
    const offChanged = bridge.on("changed", mine);
    const offInsights = bridge.on("insights", mine);
    return () => {
      gone = true;
      clearTimeout(timer);
      offChanged();
      offInsights();
    };
  }, [paneId, bridge]);

  if (!paneId) return <p className="small ao-dim ctx-empty">This tab has no shell.</p>;
  if (state.error) {
    const none = /no Claude session|no baseline/.test(state.error);
    return <p className="small ao-dim ctx-empty">{none ? "No Claude session in this tab." : state.error}</p>;
  }
  const answer = state.answer;
  if (!answer) return null;
  const note = !answer.tree && (
    <p className="small ao-dim cl-untracked">Outside git only Claude's own file edits show; Bash edits are not tracked.</p>
  );
  if (!answer.files.length)
    return (
      <>
        {note}
        <p className="small ao-dim ctx-empty">No changes yet.</p>
      </>
    );
  const open = (c: ClaudeChange) => {
    if (!app) return;
    setOpenError(undefined);
    app.openText(c.path, claudeDiff({ session: answer.session, path: c.path })).catch((e: unknown) => setOpenError(message(e)));
  };
  return (
    <>
      {openError && (
        <p className="small app-error" role="alert">
          {openError}
        </p>
      )}
      {note}
      <ul className="ctx-list cl-changes" aria-label="What Claude changed">
        {answer.files.map((c) => {
          const mark = changeMark(c);
          const { dir, name } = splitUnder(c.path, answer.base);
          return (
            <li key={c.path} className="ctx-item cl-change" data-path={c.path}>
              <span className={`cl-change-mark cl-${mark.text}`} title={mark.title}>
                {mark.text}
              </span>
              <button type="button" className="ctx-value link cl-path" title={c.path} disabled={!app} onClick={() => open(c)}>
                <span className="cl-name">{name}</span>
                {dir && <span className="ao-dim cl-dir"> {dir}</span>}
              </button>
              {c.none === "too large" ? (
                <span className="small ao-dim">Too large to compare</span>
              ) : c.none ? (
                <span className="small ao-dim">{c.none}</span>
              ) : c.added == null ? (
                <span className="small ao-dim">binary</span>
              ) : (
                <span className="small cl-lines">
                  <span className="cl-added">+{c.added}</span>
                  <span className="cl-removed"> −{c.removed ?? 0}</span>
                </span>
              )}
            </li>
          );
        })}
      </ul>
    </>
  );
}

const NOTES = {
  reading: "Reading the file before the session…",
  created: "New file: made in this session.",
  changed: "Changes since the session started.",
};

/** A diff tab over a file Claude changed, against its text before the session. */
export function ClaudeDiffView({ source, bridge, app }: PaneViewProps) {
  const src = useMemo(() => parseClaudeDiff(source), [source]);
  return (
    <DiffPane
      readKey={source}
      read={() => (src ? bridge.call<DiffBase>("baseText", { session: src.session, path: src.path }) : Promise.reject(new Error("the tab names no file")))}
      notes={NOTES}
      renderText={app.renderText}
    />
  );
}

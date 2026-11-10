// The Changes section of the context panel (add-git-plugin 3.1): the shown tab's repository,
// read again each time the Rust half reports that the pane's facts were read.

import { useEffect, useState } from "react";
import type { PanelSectionProps } from "@terminescu/plugin-api";
import { diffView } from "./DiffView";

/** One changed file, as the Rust half's `changes` call answers it. */
export interface Change {
  path: string;
  from?: string | null;
  staged: boolean;
  unstaged: boolean;
  untracked: boolean;
  conflicted: boolean;
  added?: number | null;
  removed?: number | null;
  binary: boolean;
  /** Gone from the working tree. */
  deleted?: boolean;
}

export interface ChangesAnswer {
  root: string;
  /** Every changed file; `changes` holds the first 500. */
  total: number;
  changes: Change[];
}

/** The mark beside a file: `!` conflict, `U` untracked, `S` staged, `M` unstaged. */
export function markOf(c: Change): { text: string; title: string; kind: string } {
  if (c.conflicted) return { text: "!", title: "Conflict", kind: "conflicted" };
  if (c.untracked) return { text: "U", title: "Untracked", kind: "untracked" };
  if (c.staged && c.unstaged) return { text: "SM", title: "Staged, with unstaged edits", kind: "staged" };
  if (c.staged) return { text: "S", title: "Staged", kind: "staged" };
  return { text: "M", title: "Unstaged", kind: "unstaged" };
}

const split = (path: string) => {
  const at = path.lastIndexOf("/");
  return at < 0 ? { dir: "", name: path } : { dir: path.slice(0, at + 1), name: path.slice(at + 1) };
};

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function ChangesSection({ paneId, bridge, app }: PanelSectionProps) {
  const [state, setState] = useState<{ answer?: ChangesAnswer; error?: string }>({});
  const [openError, setOpenError] = useState<string>();
  useEffect(() => {
    setState({});
    if (!paneId) return;
    let gone = false;
    let asked = 0;
    const load = () => {
      const n = ++asked;
      const settle = (s: { answer?: ChangesAnswer; error?: string }) => !gone && n === asked && setState(s);
      bridge.call<ChangesAnswer>("changes", { pane: paneId }).then(
        (answer) => settle({ answer }),
        (e: unknown) => settle({ error: message(e) }),
      );
    };
    load();
    const off = bridge.on("facts", (p) => (p as { pane?: string } | null)?.pane === paneId && load());
    return () => {
      gone = true;
      off();
    };
  }, [paneId, bridge]);

  if (!paneId) return <p className="small ao-dim ctx-empty">This tab has no shell.</p>;
  if (state.error) return <p className="small ao-dim ctx-empty git-off">{state.error}</p>;
  const answer = state.answer;
  if (!answer) return null;
  if (!answer.changes.length) return <p className="small ao-dim ctx-empty">No changes.</p>;
  const open = (c: Change) => {
    if (!app) return;
    setOpenError(undefined);
    const root = answer.root.replace(/\/+$/, "");
    const view = diffView({ root, path: c.path, ...(c.from ? { from: c.from } : {}), ...(c.deleted ? { deleted: true } : {}) });
    app.openText(`${root}/${c.path}`, view).catch((e: unknown) => setOpenError(message(e)));
  };
  return (
    <>
      {openError && (
        <p className="small app-error git-open-error" role="alert">
          {openError}
        </p>
      )}
      {answer.total > answer.changes.length && (
        <p className="small ao-dim git-total">
          The first {answer.changes.length} of {answer.total} changed files.
        </p>
      )}
      <ul className="ctx-list git-changes" aria-label="Changes">
        {answer.changes.map((c) => {
          const mark = markOf(c);
          const { dir, name } = split(c.path);
          return (
            <li key={c.path} className="ctx-item git-change" data-path={c.path}>
              <span className={`git-mark ${mark.kind}`} title={mark.title}>
                {mark.text}
              </span>
              <button
                type="button"
                className="ctx-value link git-path"
                title={c.from ? `${c.path}, renamed from ${c.from}` : c.path}
                disabled={!app}
                onClick={() => open(c)}
              >
                <span className="git-name">{name}</span>
                {dir && <span className="ao-dim git-dir"> {dir}</span>}
              </button>
              {c.binary ? (
                <span className="small ao-dim">binary</span>
              ) : (
                <span className="small git-lines">
                  {c.added != null && <span className="git-added">+{c.added}</span>}
                  {c.removed != null && <span className="git-removed"> −{c.removed}</span>}
                </span>
              )}
            </li>
          );
        })}
      </ul>
    </>
  );
}

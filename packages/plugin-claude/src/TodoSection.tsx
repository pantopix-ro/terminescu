// The todo mirror (add-claude-insights 2.2, D5): the shown tab's Claude session's latest todo
// list, read again each time the Rust half reports that the pane's list changed.

import { useEffect, useState } from "react";
import type { PanelSectionProps } from "@terminescu/plugin-api";

export interface Todo {
  content: string;
  state: "pending" | "inProgress" | "completed";
  activeForm?: string;
}

export interface TodosAnswer {
  session: string | null;
  todos: Todo[] | null;
}

const MARK: Record<Todo["state"], { text: string; title: string }> = {
  pending: { text: "○", title: "Pending" },
  inProgress: { text: "◐", title: "In progress" },
  completed: { text: "●", title: "Done" },
};

export function TodoSection({ paneId, bridge, setHidden }: PanelSectionProps) {
  const [answer, setAnswer] = useState<TodosAnswer>();
  // A tab with no Claude session shows no plan section at all (polish-claude-insights D2).
  const none = !paneId || (answer !== undefined && !answer.session);
  const loading = !!paneId && answer === undefined;
  useEffect(() => {
    if (!loading) setHidden?.(none);
  }, [none, loading, setHidden]);
  useEffect(() => {
    setAnswer(undefined);
    if (!paneId) return;
    let gone = false;
    let asked = 0;
    const load = () => {
      const n = ++asked;
      bridge.call<TodosAnswer>("todos", { pane: paneId }).then(
        (a) => !gone && n === asked && setAnswer(a),
        () => !gone && n === asked && setAnswer({ session: null, todos: null }),
      );
    };
    load();
    const off = bridge.on("insights", (p) => (p as { pane?: string } | null)?.pane === paneId && load());
    return () => {
      gone = true;
      off();
    };
  }, [paneId, bridge]);

  if (!answer) return null;
  if (!answer.session) return <p className="small ao-dim ctx-empty">No Claude session in this tab.</p>;
  if (!answer.todos?.length) return <p className="small ao-dim ctx-empty">No todo list yet.</p>;
  return (
    <ul className="cl-todos small" aria-label="Claude's todo list">
      {answer.todos.map((t, i) => (
        <li key={i} className={`cl-todo cl-${t.state}`} data-state={t.state}>
          <span className="cl-mark" title={MARK[t.state].title} aria-label={MARK[t.state].title}>
            {MARK[t.state].text}
          </span>
          <span className="cl-text">{t.state === "inProgress" && t.activeForm ? t.activeForm : t.content}</span>
        </li>
      ))}
    </ul>
  );
}

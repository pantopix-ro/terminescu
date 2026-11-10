// A plugin's question (add-git-plugin 3.3): the app draws it, so every plugin's dialogs look and
// answer the same. Enter picks the first choice, Escape and a click outside answer null. A pick
// list's item answers with it on Enter or a double click.

import { useState } from "react";
import type { Answer, Question } from "@terminescu/plugin-api";

export interface AskDialogProps {
  question: Question;
  onAnswer(answer: Answer): void;
}

export function AskDialog({ question, onAnswer }: AskDialogProps) {
  const fields = question.fields ?? [];
  const [values, setValues] = useState<Record<string, string>>(() => Object.fromEntries(fields.map((f) => [f.id, f.value ?? ""])));
  const items = question.pick?.items ?? [];
  const [pick, setPick] = useState(items[0]?.id);
  const answer = (choice: string | null) => onAnswer({ choice, values: pick === undefined ? values : { ...values, pick } });
  const [first, ...rest] = question.choices;
  return (
    <div className="sheet-back" onPointerDown={() => answer(null)}>
      <form
        className="ao-frame x-dialog ask-dialog"
        role="alertdialog"
        aria-modal="true"
        aria-label={question.title}
        onPointerDown={(e) => e.stopPropagation()}
        onSubmit={(e) => {
          e.preventDefault();
          answer(first?.id ?? null);
        }}
        onKeyDown={(e) => {
          if (e.key !== "Escape") return;
          e.preventDefault();
          answer(null);
        }}
      >
        <div className="ao-frame-head">
          <span className="ao-pixel">{question.title}</span>
        </div>
        <div className="ao-frame-body x-body">
          {question.text && <p className="ask-text">{question.text}</p>}
          {fields.map((f, i) => (
            <label key={f.id} className="x-row ask-row">
              <span className="small ao-dim">{f.label}</span>
              <input
                className="ao-input"
                name={f.id}
                autoFocus={i === 0}
                spellCheck={false}
                autoComplete="off"
                placeholder={f.placeholder}
                value={values[f.id] ?? ""}
                onChange={(e) => setValues((v) => ({ ...v, [f.id]: e.target.value }))}
              />
            </label>
          ))}
          {question.pick && (
            <div className="ask-pick" role="radiogroup" aria-label={question.pick.label}>
              {items.map((it, i) => (
                <button
                  key={it.id}
                  type="button"
                  role="radio"
                  aria-checked={pick === it.id}
                  className={`ask-item${pick === it.id ? " on" : ""}`}
                  autoFocus={i === 0 && fields.length === 0}
                  onClick={() => setPick(it.id)}
                  onDoubleClick={() => onAnswer({ choice: first?.id ?? null, values: { ...values, pick: it.id } })}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      return onAnswer({ choice: first?.id ?? null, values: { ...values, pick: it.id } });
                    }
                    const step = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
                    if (!step) return;
                    e.preventDefault();
                    const next = items[Math.min(items.length - 1, Math.max(0, i + step))];
                    if (!next) return;
                    setPick(next.id);
                    (e.currentTarget.parentElement?.children[items.indexOf(next)] as HTMLElement | undefined)?.focus();
                  }}
                >
                  <span className="grow">{it.label}</span>
                  {it.detail && <span className="small ao-dim">{it.detail}</span>}
                </button>
              ))}
            </div>
          )}
          {question.detail && <code className="small ao-dim ask-detail">{question.detail}</code>}
          <div className="close-actions">
            <span className="grow" />
            <button type="button" className="ao-btn ghost" onClick={() => answer(null)}>
              Cancel
            </button>
            {rest.map((c) => (
              <button key={c.id} type="button" className={`ao-btn ${c.danger ? "danger" : "ghost"}`} onClick={() => answer(c.id)}>
                {c.label}
              </button>
            ))}
            {first && (
              <button type="submit" className={`ao-btn ${first.danger ? "danger" : "primary"}`} autoFocus={fields.length === 0 && items.length === 0}>
                {first.label}
              </button>
            )}
          </div>
        </div>
      </form>
    </div>
  );
}

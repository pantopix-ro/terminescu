// A plugin's question (add-git-plugin 3.3): the app draws it, so every plugin's dialogs look and
// answer the same. Enter picks the first choice, Escape and a click outside answer null.

import { useState } from "react";
import type { Answer, Question } from "@terminescu/plugin-api";

export interface AskDialogProps {
  question: Question;
  onAnswer(answer: Answer): void;
}

export function AskDialog({ question, onAnswer }: AskDialogProps) {
  const fields = question.fields ?? [];
  const [values, setValues] = useState<Record<string, string>>(() => Object.fromEntries(fields.map((f) => [f.id, f.value ?? ""])));
  const answer = (choice: string | null) => onAnswer({ choice, values });
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
              <button type="submit" className={`ao-btn ${first.danger ? "danger" : "primary"}`} autoFocus={fields.length === 0}>
                {first.label}
              </button>
            )}
          </div>
        </div>
      </form>
    </div>
  );
}

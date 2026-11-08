import { isVariableName, type NewPrompt, type Pin, type Prompt, type Variable } from "@terminescu/host";
import { SavePromptForm } from "./SavePromptForm";
import { useEffect, useMemo, useRef, useState } from "react";
import { insertText } from "./names";
import { renderPrompt } from "./insert";

export interface InsertDialogProps {
  prompts: Prompt[];
  variables: Variable[];
  pins: Pin[];
  /** Writes the text to the focused terminal; Enter follows only with `send`. */
  onInsert(text: string, send: boolean, prompt?: Prompt): void;
  /** Saves a value typed for a missing variable to the side tab's context. */
  onSaveVariable(name: string, value: string): Promise<unknown>;
  /** Save as prompt (D14); resolves once saved. */
  onSavePrompt(prompt: NewPrompt): Promise<unknown>;
  onError(message: string): void;
  onClose(): void;
}

type Choice = { kind: "prompt"; prompt: Prompt } | { kind: "variable"; variable: Variable };

function matches(c: Choice, q: string): boolean {
  if (!q) return true;
  const hay =
    c.kind === "prompt"
      ? `${c.prompt.name} ${c.prompt.key} ${c.prompt.tags.map((t) => `#${t}`).join(" ")} ${c.prompt.body}`
      : `{{${c.variable.name}}} ${c.variable.mode === "secret" ? "" : (c.variable.value ?? "")}`;
  const low = hay.toLowerCase();
  return q.toLowerCase().split(/\s+/).filter(Boolean).every((w) => low.includes(w));
}

/** The prompt step: missing variables asked inline, the text editable before it goes in. */
function PromptStep({ prompt, props, onBack }: { prompt: Prompt; props: InsertDialogProps; onBack(): void }) {
  const { variables, pins } = props;
  const [extra, setExtra] = useState<Record<string, string>>({});
  const [save, setSave] = useState<Record<string, boolean>>({});
  const missing = useMemo(() => renderPrompt(prompt.body, variables, pins).missing, [prompt.body, variables, pins]);
  const rendered = renderPrompt(prompt.body, variables, pins, extra);
  // Once edited by hand, the text stops following the fields.
  const [edited, setEdited] = useState<string>();
  const text = edited ?? rendered.text;
  const first = useRef<HTMLInputElement & HTMLTextAreaElement>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState<string>();
  useEffect(() => {
    if (!saving) first.current?.focus();
  }, [saving]);

  const go = async (send: boolean) => {
    if (!text.trim()) return;
    for (const name of missing) {
      const value = extra[name]?.trim();
      if (save[name] && value) {
        // Saved mode, like a typed variable; off unless ticked, so nothing reaches disk unasked.
        try {
          await props.onSaveVariable(name, value);
        } catch (e) {
          return props.onError(String(e));
        }
      }
    }
    props.onInsert(text, send, prompt);
  };
  const keys = (e: React.KeyboardEvent) => {
    if (e.metaKey && e.key === "Enter") {
      e.preventDefault();
      void go(e.shiftKey);
    }
  };

  if (saving)
    return (
      <SavePromptForm
        text={text}
        variables={variables}
        onSave={(p) =>
          props.onSavePrompt(p).then(() => {
            setSaving(false);
            setSaved(p.name);
          })
        }
        onCancel={() => setSaving(false)}
      />
    );
  return (
    <div className="ins-step" onKeyDown={keys}>
      <div className="ins-head">
        <button type="button" className="link small" onClick={onBack}>
          ‹ all
        </button>
        <span className="ins-title">{prompt.name}</span>
        <span className="small ao-dim">v{prompt.version}</span>
      </div>
      {missing.length > 0 ? (
        <div className="ins-missing" role="group" aria-label="Missing variables">
          {missing.map((name, i) => (
            <label key={name} className="ins-var">
              <span className="ctx-name">{`{{${name}}}`}</span>
              <input
                ref={i === 0 ? first : undefined}
                className="ao-input"
                aria-label={`Value for ${name}`}
                value={extra[name] ?? ""}
                onChange={(e) => setExtra((x) => ({ ...x, [name]: e.target.value }))}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.metaKey) {
                    e.preventDefault();
                    const next = e.currentTarget.closest(".ins-step")?.querySelectorAll<HTMLInputElement>(".ins-var .ao-input")[i + 1];
                    (next ?? e.currentTarget.closest(".ins-step")?.querySelector<HTMLTextAreaElement>("textarea"))?.focus();
                  }
                }}
              />
              {isVariableName(name) && (
                <span className="small ao-dim ins-save">
                  <input
                    type="checkbox"
                    aria-label={`Save ${name} to the context`}
                    checked={save[name] ?? false}
                    onChange={(e) => setSave((s) => ({ ...s, [name]: e.target.checked }))}
                  />{" "}
                  save
                </span>
              )}
            </label>
          ))}
        </div>
      ) : null}
      <textarea
        ref={missing.length ? undefined : first}
        className="ao-input ins-text"
        aria-label="Text to insert"
        spellCheck={false}
        value={text}
        onChange={(e) => setEdited(e.target.value)}
      />
      <div className="pl-row">
        <span className="small ao-dim grow">
          {saved
            ? `saved as ${saved}`
            : rendered.missing.length
              ? `still empty: ${rendered.missing.map((n) => `{{${n}}}`).join(" ")}`
              : "nothing is sent until you choose to"}
        </span>
        <button type="button" className="ao-btn ghost" onClick={() => setSaving(true)}>
          Save as prompt
        </button>
        <button type="button" className="ao-btn ghost" onClick={() => void go(true)}>
          Insert and send ⌘⇧↵
        </button>
        <button type="button" className="ao-btn primary" onClick={() => void go(false)}>
          Insert ⌘↵
        </button>
      </div>
    </div>
  );
}

/** `⌘I`: prompts and variables of the side tab, inserted into the focused terminal (D13). */
export function InsertDialog(props: InsertDialogProps) {
  const { prompts, variables, onClose } = props;
  const [query, setQuery] = useState("");
  const [at, setAt] = useState(0);
  const [prompt, setPrompt] = useState<Prompt>();
  const search = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (!prompt) search.current?.focus();
  }, [prompt]);
  const choices: Choice[] = [
    ...prompts.map((p) => ({ kind: "prompt", prompt: p }) as Choice),
    ...variables.map((v) => ({ kind: "variable", variable: v }) as Choice),
  ].filter((c) => matches(c, query.trim()));
  const sel = Math.min(at, Math.max(0, choices.length - 1));
  const choose = (c: Choice | undefined) => {
    if (!c) return;
    if (c.kind === "prompt") setPrompt(c.prompt);
    else props.onInsert(insertText(c.variable), false);
  };

  return (
    <div className="sheet-back" onPointerDown={onClose}>
      <div
        className="ao-frame insert-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="Insert"
        onPointerDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            if (prompt) setPrompt(undefined);
            else onClose();
          }
        }}
      >
        <div className="ao-frame-body">
          {prompt ? (
            <PromptStep prompt={prompt} props={props} onBack={() => setPrompt(undefined)} />
          ) : (
            <>
              <input
                ref={search}
                className="ao-input"
                aria-label="Search prompts and variables"
                placeholder="insert a prompt or a variable"
                value={query}
                onChange={(e) => {
                  setQuery(e.target.value);
                  setAt(0);
                }}
                onKeyDown={(e) => {
                  if (e.key === "ArrowDown") {
                    e.preventDefault();
                    setAt(Math.min(sel + 1, choices.length - 1));
                  } else if (e.key === "ArrowUp") {
                    e.preventDefault();
                    setAt(Math.max(sel - 1, 0));
                  } else if (e.key === "Enter") {
                    e.preventDefault();
                    choose(choices[sel]);
                  }
                }}
              />
              <ul className="ins-list" role="listbox" aria-label="Prompts and variables">
                {choices.map((c, i) => (
                  <li
                    key={c.kind === "prompt" ? `p:${c.prompt.id}` : `v:${c.variable.name}`}
                    role="option"
                    aria-selected={i === sel}
                    className={`palette-item ins-item ${c.kind}`}
                    data-selected={i === sel}
                    onPointerEnter={() => setAt(i)}
                    onClick={() => choose(c)}
                  >
                    {c.kind === "prompt" ? (
                      <>
                        <span className="ins-title">{c.prompt.name}</span>
                        <span className="small ao-dim grow">{c.prompt.variables.map((v) => `{{${v}}}`).join(" ")}</span>
                        <span className="small ao-dim">prompt</span>
                      </>
                    ) : (
                      <>
                        <span className="ctx-name">{`{{${c.variable.name}}}`}</span>
                        <span className="small ao-dim grow ins-value">
                          {c.variable.mode === "secret" ? `inserts as $${c.variable.name}` : c.variable.value}
                        </span>
                        <span className="small ao-dim">{c.variable.mode}</span>
                      </>
                    )}
                  </li>
                ))}
                {choices.length === 0 && <li className="small ao-dim">Nothing matches.</li>}
              </ul>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

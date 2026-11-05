import type { NewPrompt, Variable } from "@terminescu/host";
import { useEffect, useRef, useState } from "react";
import { detectVariables, templatize } from "./detect";

export interface SavePromptFormProps {
  text: string;
  variables: Variable[];
  onSave(prompt: NewPrompt): Promise<unknown>;
  onCancel(): void;
}

/** Save as prompt (D14): offers `{{name}}` for every context value found in the text. */
export function SavePromptForm({ text, variables, onSave, onCancel }: SavePromptFormProps) {
  const found = detectVariables(text, variables);
  const [name, setName] = useState("");
  const [tags, setTags] = useState("");
  const [off, setOff] = useState<string[]>([]);
  const [error, setError] = useState<string>();
  const first = useRef<HTMLInputElement>(null);
  useEffect(() => first.current?.focus(), []);
  const body = templatize(
    text,
    found.filter((f) => !off.includes(f.name)),
  );
  return (
    <form
      className="save-prompt"
      aria-label="Save as prompt"
      onSubmit={(e) => {
        e.preventDefault();
        setError(undefined);
        void onSave({ name, tags: tags.split(/[,\s]+/).filter(Boolean), body }).catch((err: unknown) => setError(String(err)));
      }}
    >
      <input ref={first} className="ao-input" aria-label="Prompt name" placeholder="prompt name" value={name} onChange={(e) => setName(e.target.value)} />
      <input className="ao-input" aria-label="Tags" placeholder="tags, comma separated" value={tags} onChange={(e) => setTags(e.target.value)} />
      {found.length > 0 && (
        <div className="sp-found" role="group" aria-label="Variables found">
          {found.map((f) => (
            <label key={f.name} className="small">
              <input
                type="checkbox"
                aria-label={`Use {{${f.name}}}`}
                checked={!off.includes(f.name)}
                onChange={(e) => setOff((o) => (e.target.checked ? o.filter((n) => n !== f.name) : [...o, f.name]))}
              />{" "}
              <span className="ctx-name">{`{{${f.name}}}`}</span> <span className="ao-dim">for</span> <code>{f.value}</code>
            </label>
          ))}
        </div>
      )}
      <textarea className="ao-input sp-body" aria-label="Prompt preview" readOnly value={body} />
      {error && (
        <p className="small app-error-inline" role="alert">
          {error}
        </p>
      )}
      <div className="pl-row">
        <span className="grow" />
        <button type="button" className="ao-btn ghost" onClick={onCancel}>
          cancel
        </button>
        <button type="submit" className="ao-btn primary">
          Save prompt
        </button>
      </div>
    </form>
  );
}

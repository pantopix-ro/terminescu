import { VAR_MODES, type Item, type ItemKind, type NewVariable, type Pin, type Prompt, type Variable, type VarMode, type VariableChange } from "@terminescu/host";
import { useEffect, useRef, useState } from "react";
import { parseTtl, timeLeft } from "./names";

export type ContextMode = "pinned" | "auto";

export interface ContextPanelProps {
  mode: ContextMode;
  onMode(mode: ContextMode): void;
  /** The active side tab's name; the context belongs to it. */
  owner: string | undefined;
  variables?: Variable[];
  items?: Item[];
  pins?: Pin[];
  /** Scrolls a transcript or terminal to the pin (D12). */
  onOpenPin?(pin: Pin): void;
  onRemovePin?(pin: Pin): void;
  /** Each resolves or rejects; the panel shows a rejection next to the form. */
  onAddVariable?(variable: NewVariable): Promise<unknown>;
  onChangeVariable?(name: string, change: VariableChange): Promise<unknown>;
  onRemoveVariable?(name: string): Promise<unknown>;
  /** Types the variable into the focused terminal; absent when no terminal has focus. */
  onInsertVariable?(variable: Variable): void;
  /** Opens a saved or session variable in a text tab (D27); secrets never open. */
  onEditVariable?(variable: Variable): void;
  onAddItem?(kind: ItemKind, value: string): Promise<unknown>;
  onRemoveItem?(id: string): Promise<unknown>;
  onOpenUrl?(url: string): void;
  /** The library, last used first (D14). */
  prompts?: Prompt[];
  /** Opens the library at a prompt, or at its start. */
  onOpenPrompt?(key?: string): void;
}

const SECTIONS = ["Context", "Pins", "Prompts"] as const;
type Section = (typeof SECTIONS)[number];

/** How long the pointer rests on the edge before the panel slides in. */
export const HOVER_MS = 150;

const MODE_NAME: Record<VarMode, string> = { saved: "Saved", session: "Session", secret: "Secret" };

/** A rerender every 20 s, so TTLs count down. */
function useMinuteTick(on: boolean) {
  const [, setTick] = useState(0);
  useEffect(() => {
    if (!on) return;
    const t = setInterval(() => setTick((n) => n + 1), 20_000);
    return () => clearInterval(t);
  }, [on]);
}

function metaOf(v: Variable): string {
  const left = v.expiresAt != null ? ` · ${timeLeft(v.expiresAt)}` : "";
  if (v.mode === "secret") return `inserts as $${v.name}${left}`;
  if (v.mode === "session") return `not stored · gone on reboot${left}`;
  return v.source ?? "";
}

function VariableRow({ v, props, fail }: { v: Variable; props: ContextPanelProps; fail(e: unknown): void }) {
  const [editing, setEditing] = useState<string>();
  const secret = v.mode === "secret";
  const commit = () => {
    if (editing === undefined) return;
    const value = editing;
    setEditing(undefined);
    if (value.trim() && value !== v.value) void props.onChangeVariable?.(v.name, { value })?.catch(fail);
  };
  return (
    <li className={`ctx-var ${v.mode}`} data-var={v.name} data-mode={v.mode}>
      <span className="ctx-name">{`{{${v.name}}}`}</span>
      {editing !== undefined ? (
        <input
          className="ao-input"
          aria-label={`Value of ${v.name}`}
          type={secret ? "password" : "text"}
          autoFocus
          placeholder={secret ? "new value" : undefined}
          value={editing}
          onChange={(e) => setEditing(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
            if (e.key === "Escape") {
              e.stopPropagation();
              setEditing(undefined);
            }
          }}
        />
      ) : (
        <button
          type="button"
          className={`ctx-value small link-plain${secret ? " masked" : ""}`}
          title={secret ? "Set a new value" : (v.value ?? "")}
          onClick={() => setEditing(secret ? "" : (v.value ?? ""))}
        >
          {secret ? "••••••••••••" : v.value}
        </button>
      )}
      <span className="ctx-row-acts">
        <select
          aria-label={`Storage of ${v.name}`}
          value={v.mode}
          onChange={(e) => void props.onChangeVariable?.(v.name, { mode: e.target.value as VarMode })?.catch(fail)}
        >
          {VAR_MODES.map((m) => (
            <option key={m} value={m} disabled={secret && m !== "secret"}>
              {MODE_NAME[m]}
            </option>
          ))}
        </select>
        {props.onInsertVariable && (
          <button
            type="button"
            className="ao-btn ghost ctx-insert"
            aria-label={`Insert ${v.name}`}
            title={secret ? `Types $${v.name}; the value stays in the environment` : "Types the value, without Enter"}
            onClick={() => props.onInsertVariable?.(v)}
          >
            insert
          </button>
        )}
        {props.onEditVariable && !secret && (
          <button type="button" className="ao-btn ghost" aria-label={`Edit ${v.name} in a text tab`} onClick={() => props.onEditVariable?.(v)}>
            edit
          </button>
        )}
        <button type="button" className="ctx-pin-x" aria-label={`Remove ${v.name}`} onClick={() => void props.onRemoveVariable?.(v.name)?.catch(fail)}>
          ×
        </button>
      </span>
      <span className="ctx-meta">{metaOf(v)}</span>
    </li>
  );
}

function AddVariable({ props, fail, done }: { props: ContextPanelProps; fail(e: unknown): void; done(): void }) {
  const [name, setName] = useState("");
  const [value, setValue] = useState("");
  const [mode, setMode] = useState<VarMode>("saved");
  const [ttl, setTtl] = useState("");
  return (
    <form
      className="ctx-add"
      aria-label="Add a variable"
      onSubmit={(e) => {
        e.preventDefault();
        const ttlSecs = mode === "saved" || !ttl.trim() ? null : parseTtl(ttl);
        if (ttlSecs === undefined) return fail("TTL reads like 30m, 1h or 2d");
        void props
          .onAddVariable?.({ name: name.trim(), value, mode, ttlSecs, source: "typed" })
          ?.then(done, fail);
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          done();
        }
      }}
    >
      <input className="ao-input" aria-label="Name" placeholder="name" autoFocus spellCheck={false} value={name} onChange={(e) => setName(e.target.value)} />
      <select aria-label="Storage" value={mode} onChange={(e) => setMode(e.target.value as VarMode)}>
        {VAR_MODES.map((m) => (
          <option key={m} value={m}>
            {MODE_NAME[m]}
          </option>
        ))}
      </select>
      <input
        className="ao-input wide"
        aria-label="Value"
        placeholder="value"
        type={mode === "secret" ? "password" : "text"}
        value={value}
        onChange={(e) => setValue(e.target.value)}
      />
      {mode !== "saved" && <input className="ao-input" aria-label="TTL" placeholder="TTL: none, or 1h" value={ttl} onChange={(e) => setTtl(e.target.value)} />}
      <button type="submit" className="ao-btn primary">
        Add
      </button>
    </form>
  );
}

function AddItem({ props, fail }: { props: ContextPanelProps; fail(e: unknown): void }) {
  const [value, setValue] = useState("");
  return (
    <form
      className="ctx-add"
      aria-label="Add a link or text"
      onSubmit={(e) => {
        e.preventDefault();
        const v = value.trim();
        if (!v) return;
        const kind: ItemKind = /^https?:\/\/\S+$/.test(v) ? "link" : "text";
        void props.onAddItem?.(kind, v)?.then(() => setValue(""), fail);
      }}
    >
      <input className="ao-input" aria-label="Link or text" placeholder="paste a link or a note" value={value} onChange={(e) => setValue(e.target.value)} />
      <button type="submit" className="ao-btn ghost">
        Add
      </button>
    </form>
  );
}

function Items(props: ContextPanelProps & { section: Section }) {
  const { section, variables = [], items = [], pins = [], onOpenPin, onRemovePin } = props;
  const [adding, setAdding] = useState(false);
  const [error, setError] = useState<string>();
  const fail = (e: unknown) => setError(String(e));
  useMinuteTick(section === "Context" && variables.some((v) => v.expiresAt != null));
  if (section === "Context") {
    return (
      <>
        <div className="ctx-group">
          <span className="ao-pixel ao-dim">Variables</span>
          <span className="small ao-dim">{variables.length}</span>
          <span className="grow" />
          <button type="button" className="ao-btn ghost ctx-mode" onClick={() => setAdding((a) => !a)} aria-expanded={adding}>
            + variable
          </button>
        </div>
        {adding && <AddVariable props={props} fail={fail} done={() => setAdding(false)} />}
        {error && (
          <p className="small app-error ctx-empty" role="alert" onClick={() => setError(undefined)}>
            {error}
          </p>
        )}
        {variables.length ? (
          <ul className="ctx-list" aria-label="Variables">
            {variables.map((v) => (
              <VariableRow key={v.id} v={v} props={props} fail={fail} />
            ))}
          </ul>
        ) : (
          <p className="small ao-dim ctx-empty">No variables yet. Select text and press ⌘E.</p>
        )}
        <div className="ctx-group">
          <span className="ao-pixel ao-dim">Links and text</span>
          <span className="small ao-dim">{items.length}</span>
        </div>
        <AddItem props={props} fail={fail} />
        <ul className="ctx-list" aria-label="Links and text">
          {items.map((i) => (
            <li key={i.id} className={`ctx-item ${i.kind}`}>
              {i.kind === "link" ? (
                <button type="button" className="ctx-value link" title={i.value} onClick={() => props.onOpenUrl?.(i.value)}>
                  {i.value.replace(/^https?:\/\//, "")}
                </button>
              ) : (
                <span className="ctx-value" title={i.value}>
                  {i.value}
                </span>
              )}
              <button type="button" className="ctx-pin-x" aria-label="Remove" onClick={() => void props.onRemoveItem?.(i.id)?.catch(fail)}>
                ×
              </button>
            </li>
          ))}
        </ul>
      </>
    );
  }
  if (section === "Pins" && pins.length) {
    return (
      <ul className="ctx-list">
        {pins.map((p) => (
          <li key={p.id} className="ctx-pin" data-pin={p.id} data-kind={p.paneId ? "terminal" : "transcript"}>
            <button type="button" className="ctx-pin-open" title={p.paneId ? "Show in the terminal" : "Show in the transcript"} onClick={() => onOpenPin?.(p)}>
              <i className={`ao-i x1 ${p.paneId ? "ao-i-term" : "ao-i-pin"}`} />
              <span className="ctx-quote">{p.exact}</span>
            </button>
            <button type="button" className="ctx-pin-x" aria-label="Remove pin" onClick={() => onRemovePin?.(p)}>
              ×
            </button>
          </li>
        ))}
      </ul>
    );
  }
  if (section === "Prompts") {
    const { prompts = [], onOpenPrompt } = props;
    return (
      <>
        <div className="ctx-group">
          <span className="ao-pixel ao-dim">Prompts</span>
          <span className="small ao-dim">{prompts.length}</span>
          <span className="grow" />
          <button type="button" className="ao-btn ghost ctx-mode" onClick={() => onOpenPrompt?.()}>
            library ⌘⇧I
          </button>
        </div>
        <ul className="ctx-list" aria-label="Prompts">
          {prompts.map((p) => (
            <li key={p.id} className="ctx-item" data-prompt={p.key}>
              <button type="button" className="ctx-value link-plain" title={p.body} onClick={() => onOpenPrompt?.(p.key)}>
                {p.name}
              </button>
              <span className="small ao-dim">{p.variables.map((v) => `{{${v}}}`).join(" ")}</span>
            </li>
          ))}
        </ul>
        {prompts.length === 0 && <p className="small ao-dim ctx-empty">No prompts yet.</p>}
      </>
    );
  }
  return <p className="small ao-dim ctx-empty">No pins yet.</p>;
}

function Body(props: ContextPanelProps) {
  const { mode, onMode, owner } = props;
  const [section, setSection] = useState<Section>("Context");
  return (
    <>
      <div className="ctx-head">
        <i className="ao-i ao-i-var" />
        <span className="ao-pixel ao-dim">Context</span>
        <span className="small ao-dim ctx-owner">{owner}</span>
        <span className="grow" />
        <button
          type="button"
          className="ao-btn ghost ctx-mode"
          aria-label={mode === "pinned" ? "Auto-hide context" : "Pin context"}
          onClick={() => onMode(mode === "pinned" ? "auto" : "pinned")}
        >
          <i className="ao-i ao-i-pin" />
          <span className="small">{mode === "pinned" ? "auto-hide" : "pin"}</span>
        </button>
      </div>
      <div className="ctx-tabs" role="tablist">
        {SECTIONS.map((s) => (
          <button
            key={s}
            type="button"
            role="tab"
            aria-selected={s === section}
            className={`ctx-tab${s === section ? " active" : ""}`}
            onClick={() => setSection(s)}
          >
            {s}
          </button>
        ))}
      </div>
      <div className="ctx-scroll">
        <Items {...props} section={section} />
      </div>
    </>
  );
}

/**
 * The side tab's context (D26). Pinned, it takes a column. Auto-hidden, a 12px edge stays; a
 * short hover slides the panel in over the panes, so no pane changes size.
 */
export function ContextPanel(props: ContextPanelProps) {
  const [shown, setShown] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  useEffect(() => setShown(false), [props.mode]);

  if (props.mode === "pinned") {
    return (
      <aside className="ctx-pinned" aria-label="Context">
        <Body {...props} />
      </aside>
    );
  }
  const enter = () => {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setShown(true), HOVER_MS);
  };
  const leave = () => {
    clearTimeout(timer.current);
    setShown(false);
  };
  return (
    <div className="ctx-auto" onPointerLeave={leave}>
      <div className={`ao-edge${shown ? " hot" : ""}`} aria-label="Context, auto-hidden" onPointerEnter={enter}>
        <span className="ao-pixel">Context</span>
      </div>
      {shown && (
        <aside className="ao-ctx-over ctx-over" aria-label="Context" onPointerEnter={() => clearTimeout(timer.current)}>
          <Body {...props} />
        </aside>
      )}
    </div>
  );
}

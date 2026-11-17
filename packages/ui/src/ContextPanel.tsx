import { VAR_MODES, type Item, type ItemKind, type NewVariable, type Pin, type Prompt, type TouchedFile, type Variable, type VarMode, type VariableChange } from "@terminescu/host";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { parseTtl, timeAgo, timeLeft } from "./names";
import { Tip, useTip } from "./Tip";
import { PromptTip } from "./PromptTip";
import { tipSlots } from "./prompt-tip";

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
  /** Files Claude wrote or edited in this side tab, last touched first (D30). */
  files?: TouchedFile[];
  /** Opens a touched file in a text tab. */
  onOpenFile?(file: TouchedFile): void;
  onForgetFile?(id: string): Promise<unknown>;
  /** The library, last used first (D14). */
  prompts?: Prompt[];
  /** Opens the library at a prompt, or at its start. A right click on a prompt calls it. */
  onOpenPrompt?(key?: string): void;
  /** A left click on a prompt: insert it filled, or ask for what is missing (refine-context-panel D6). */
  onUsePrompt?(prompt: Prompt): void;
  /** The Prompts head `+`: the library on a new prompt. */
  onNewPrompt?(): void;
  /** Sections the enabled plugins add after the app's, each drawn by its plugin (D7). */
  sections?: readonly PanelPart[];
}

/** A plugin's section: the panel draws its head; the plugin draws the body. */
export interface PanelPart {
  key: string;
  title: string;
  body: ReactNode;
  /** The plugin hid the section; its body stays mounted. */
  hidden?: boolean;
  /** The count the plugin reported for the head (`setCount`). */
  count?: number;
}

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

/** A variable a plugin provides: it shows and inserts, but only the plugin changes it. */
const provided = (v: Variable) => v.id.startsWith("plugin:");

/** The mode line of a variable's tooltip: storage, and what is left of a TTL. */
function modeLine(v: Variable): string {
  const left = v.expiresAt != null ? ` · ${timeLeft(v.expiresAt)}` : "";
  if (v.mode === "secret") return `inserts as $${v.name}${left}`;
  if (v.mode === "session") return `not stored · gone on reboot${left}`;
  return `saved${left}`;
}

/** The tooltip of a variable (D4): its value, never a secret's, then the mode and the source. */
function varTip(v: Variable): { name: string; lines: string[] } {
  if (provided(v)) return { name: v.value ?? "", lines: [`from ${v.source?.replace(/^plugin:/, "") ?? "a plugin"} · read-only`] };
  const source = v.source ? [`from ${v.source}`] : [];
  if (v.mode === "secret") return { name: modeLine(v), lines: source };
  return { name: v.value || "(empty)", lines: [modeLine(v), ...source] };
}

type Hover = { onPointerEnter(e: React.PointerEvent<HTMLElement>): void; onPointerLeave(): void; onFocus(e: React.FocusEvent<HTMLElement>): void; onBlur(): void };

function VariableRow({ v, props, fail, hover }: { v: Variable; props: ContextPanelProps; fail(e: unknown): void; hover(v: Variable): Hover }) {
  const [editing, setEditing] = useState<string>();
  const secret = v.mode === "secret";
  const tip = hover(v);
  if (provided(v))
    return (
      <li className="ctx-var provided" data-var={v.name} data-mode={v.mode} data-provided="" {...tip}>
        <span className="ctx-name">{`{{${v.name}}}`}</span>
        <span className="ctx-value small">
          {v.value}
        </span>
        <span className="ctx-row-acts">
          {props.onInsertVariable && (
            <button type="button" className="ao-btn ghost ctx-insert" aria-label={`Insert ${v.name}`} title="Types the value, without Enter" onClick={() => props.onInsertVariable?.(v)}>
              ↵
            </button>
          )}
        </span>
      </li>
    );
  const commit = () => {
    if (editing === undefined) return;
    const value = editing;
    setEditing(undefined);
    if (value.trim() && value !== v.value) void props.onChangeVariable?.(v.name, { value })?.catch(fail);
  };
  return (
    <li className={`ctx-var ${v.mode}`} data-var={v.name} data-mode={v.mode} {...tip}>
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
            ↵
          </button>
        )}
        {props.onEditVariable && !secret && (
          <button type="button" className="ao-btn ghost" aria-label={`Edit ${v.name} in a text tab`} title="Edit in a text tab" onClick={() => props.onEditVariable?.(v)}>
            ✎
          </button>
        )}
        <button type="button" className="ctx-pin-x" aria-label={`Remove ${v.name}`} onClick={() => void props.onRemoveVariable?.(v.name)?.catch(fail)}>
          ×
        </button>
      </span>
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

function AddItem({ props, fail, done }: { props: ContextPanelProps; fail(e: unknown): void; done(): void }) {
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
        void props.onAddItem?.(kind, v)?.then(done, fail);
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          done();
        }
      }}
    >
      <input className="ao-input" aria-label="Link or text" placeholder="paste a link or a note" autoFocus value={value} onChange={(e) => setValue(e.target.value)} />
      <button type="submit" className="ao-btn ghost">
        Add
      </button>
    </form>
  );
}

/** Links on one host differ at the end: the end always shows, the middle gives way. */
export const LINK_TAIL = 18;
function LinkText({ url }: { url: string }) {
  const bare = url.replace(/^https?:\/\//, "");
  if (bare.length <= LINK_TAIL * 2) return <span className="ctx-link-head">{bare}</span>;
  return (
    <>
      <span className="ctx-link-head">{bare.slice(0, -LINK_TAIL)}</span>
      <span className="ctx-link-tail">{bare.slice(-LINK_TAIL)}</span>
    </>
  );
}

/** One head row for every section: the title, the count, and a `+` where there is an add action (D1). */
function SectionHead({ title, count, onAdd, addLabel, adding }: { title: string; count?: number; onAdd?(): void; addLabel?: string; adding?: boolean }) {
  return (
    <div className="ctx-sec-head">
      <h3 className="ao-pixel">{title}</h3>
      {count !== undefined && <span className="ctx-count small ao-dim">{count}</span>}
      <span className="grow" />
      {onAdd && (
        <button type="button" className="ao-btn ghost ctx-add-btn" aria-label={addLabel} aria-expanded={adding} title={addLabel} onClick={onAdd}>
          +
        </button>
      )}
    </div>
  );
}

/** The five app sections, each one head and rows (D1, D2): empty means the head alone. */
function CoreSections(props: ContextPanelProps) {
  const { variables = [], items = [], pins = [], files = [], prompts = [], onOpenPin, onRemovePin } = props;
  const [adding, setAdding] = useState<"variable" | "item">();
  const [error, setError] = useState<{ at: string; text: string }>();
  const fail = (at: string) => (e: unknown) => setError({ at, text: String(e) });
  const tips = useTip();
  useMinuteTick(files.length > 0 || variables.some((v) => v.expiresAt != null));
  const toggle = (what: "variable" | "item") => setAdding((a) => (a === what ? undefined : what));
  const err = (at: string) =>
    error?.at === at && (
      <p className="small app-error ctx-empty" role="alert" onClick={() => setError(undefined)}>
        {error.text}
      </p>
    );
  const varHover = (v: Variable): Hover => {
    const show = (el: HTMLElement) => {
      const t = varTip(v);
      tips.beside(el, t.name, t.lines);
    };
    return { onPointerEnter: (e) => show(e.currentTarget), onPointerLeave: tips.hide, onFocus: (e) => show(e.currentTarget), onBlur: tips.hide };
  };
  const promptTip = (el: HTMLElement, p: Prompt) => tips.beside(el, p.name, undefined, <PromptTip parts={tipSlots(p.body, variables, pins)} />);
  return (
    <>
      <section className="ctx-section" aria-label="Variables" data-section="Variables">
        <SectionHead title="Variables" count={variables.length} onAdd={() => toggle("variable")} addLabel="Add a variable" adding={adding === "variable"} />
        {adding === "variable" && <AddVariable props={props} fail={fail("Variables")} done={() => setAdding(undefined)} />}
        {err("Variables")}
        {variables.length > 0 && (
          <ul className="ctx-list" aria-label="Variables">
            {variables.map((v) => (
              <VariableRow key={v.id} v={v} props={props} fail={fail("Variables")} hover={varHover} />
            ))}
          </ul>
        )}
      </section>
      <section className="ctx-section" aria-label="Links and text" data-section="Links and text">
        <SectionHead title="Links and text" count={items.length} onAdd={() => toggle("item")} addLabel="Add a link or text" adding={adding === "item"} />
        {adding === "item" && <AddItem props={props} fail={fail("Links and text")} done={() => setAdding(undefined)} />}
        {err("Links and text")}
        {items.length > 0 && (
          <ul className="ctx-list" aria-label="Links and text">
            {items.map((i) => (
              <li key={i.id} className={`ctx-item ${i.kind}`}>
                {i.kind === "link" ? (
                  <button type="button" className="ctx-value link" title={i.value} onClick={() => props.onOpenUrl?.(i.value)}>
                    <LinkText url={i.value} />
                  </button>
                ) : (
                  <span className="ctx-value" title={i.value}>
                    {i.value}
                  </span>
                )}
                <button type="button" className="ctx-pin-x" aria-label="Remove" onClick={() => void props.onRemoveItem?.(i.id)?.catch(fail("Links and text"))}>
                  ×
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
      <section className="ctx-section" aria-label="Files" data-section="Files">
        <SectionHead title="Files" count={files.length} />
        {err("Files")}
        {files.length > 0 && (
          <ul className="ctx-list" aria-label="Files">
            {files.map((f) => (
              <li key={f.id} className={`ctx-item file ${f.change}`}>
                <span className={`ctx-change ${f.change}`} title={f.change}>
                  {f.change === "added" ? "A" : "M"}
                </span>
                <button type="button" className="ctx-value link" title={f.path} onClick={() => props.onOpenFile?.(f)}>
                  {f.shown}
                </button>
                <span className="small ao-dim">{timeAgo(f.updatedAt)}</span>
                <button type="button" className="ctx-pin-x" aria-label={`Forget ${f.shown}`} onClick={() => void props.onForgetFile?.(f.id)?.catch(fail("Files"))}>
                  ×
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
      {/* Hidden while empty: a pin only comes from Shell › Pin Selection (⌘⇧P). */}
      <section className="ctx-section" aria-label="Pins" data-section="Pins" hidden={pins.length === 0}>
        <SectionHead title="Pins" count={pins.length} />
        {pins.length > 0 && (
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
        )}
      </section>
      <section className="ctx-section" aria-label="Prompts" data-section="Prompts">
        <SectionHead title="Prompts" count={prompts.length} onAdd={props.onNewPrompt} addLabel="New prompt" />
        {prompts.length > 0 && (
          <ul className="ctx-list" aria-label="Prompts">
            {prompts.map((p) => (
              <li key={p.id} className="ctx-item" data-prompt={p.key}>
                <button
                  type="button"
                  className="ctx-value link-plain"
                  onPointerEnter={(e) => promptTip(e.currentTarget, p)}
                  onPointerLeave={tips.hide}
                  onFocus={(e) => promptTip(e.currentTarget, p)}
                  onBlur={tips.hide}
                  onClick={() => {
                    tips.hide();
                    props.onUsePrompt ? props.onUsePrompt(p) : props.onOpenPrompt?.(p.key);
                  }}
                  onContextMenu={(e) => {
                    // The window's Edit menu skips an event that is already handled.
                    e.preventDefault();
                    tips.hide();
                    props.onOpenPrompt?.(p.key);
                  }}
                >
                  {p.name}
                </button>
                <span className="small ao-dim">{p.variables.map((v) => `{{${v}}}`).join(" ")}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
      <Tip tip={tips.tip} />
    </>
  );
}

function Body(props: ContextPanelProps) {
  const { mode, onMode, owner } = props;
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
      <div className="ctx-scroll">
        <CoreSections {...props} />
        {props.sections?.map((s) => (
          <section key={s.key} className="ctx-section plugin" aria-label={s.title} data-section={s.key} hidden={s.hidden}>
            <SectionHead title={s.title} count={s.count} />
            {s.body}
          </section>
        ))}
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

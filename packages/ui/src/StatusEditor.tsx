import { MAX_LABEL, NEONS, type Look, type ManualStatus, type Neon, type SavedStatus, type Shown, type SideTab, type StatusConfig, type Tab } from "@terminescu/host";
import type { MenuEntry } from "./ContextMenu";
import { useEffect, useRef, useState } from "react";
import { StatusChip } from "./StatusChip";
import { builtinLook, chipOf } from "./status";
import { Swatches } from "./Swatches";

export interface StatusEditorProps {
  side: SideTab;
  /** Which status it opens on; a click on a tab's chip opens the tab's. Default: the side tab. */
  initialTarget?: "side" | "tab";
  /** The active tab of the side tab, offered as the second target. */
  tab: Tab | undefined;
  sideName: string;
  tabName: string | undefined;
  statuses: Readonly<Record<string, Shown>>;
  config: StatusConfig;
  /** Sets or clears (`null`) the manual status of a tab or side tab. */
  onSet(targetId: string, status: ManualStatus | null): Promise<unknown>;
  onSave(sideTabId: string, look: Look): Promise<SavedStatus>;
  onMakeGlobal(id: string): Promise<SavedStatus>;
  /** Opens Settings › Statuses. */
  onSettings(): void;
  onClose(): void;
  /** Task 8.34: the name, icon and neon of whatever the editor targets, edited in place. */
  onRename?(id: string, title: string | null): void;
  /** Opens the icon picker for `id` at a window point. */
  onPickIcon?(id: string, at: { x: number; y: number }): void;
  onAccent?(id: string, accent: Neon): void;
  /** The target's other actions: merge, move, close. A submenu shows as a drop-down. */
  actions?(target: "side" | "tab"): MenuEntry[];
}

/** One action of the right-click menu, as a button or, with a submenu, a drop-down. */
function ActionControl({ entry, onDone }: { entry: Exclude<MenuEntry, "-">; onDone(): void }) {
  if (entry.items) {
    const choices = entry.items.filter((i): i is Exclude<MenuEntry, "-"> => i !== "-");
    return (
      <select
        className="ao-input edit-select"
        aria-label={entry.label}
        value=""
        disabled={entry.disabled || !choices.length}
        onChange={(e) => {
          const pick = choices.find((c) => c.id === e.target.value);
          if (!pick?.run) return;
          onDone();
          pick.run();
        }}
      >
        <option value="">{entry.label}…</option>
        {choices.map((c) => (
          <option key={c.id} value={c.id} disabled={c.disabled}>
            {c.label}
          </option>
        ))}
      </select>
    );
  }
  return (
    <button
      type="button"
      className={`ao-btn ghost${entry.danger ? " danger" : ""}`}
      disabled={entry.disabled}
      onClick={() => {
        onDone();
        entry.run?.();
      }}
    >
      {entry.label}
    </button>
  );
}

const EMPTY: ManualStatus = { label: "", accent: null, background: null };

const lookOf = (s: SavedStatus): ManualStatus => ({ statusId: s.id, label: s.label, accent: s.accent, background: s.background });

/** `⌘⇧S`: the status of the focused side tab or its active tab (design board "Status"). */
export function StatusEditor(props: StatusEditorProps) {
  const { side, tab, statuses, config } = props;
  const [target, setTarget] = useState<"side" | "tab">(props.initialTarget === "tab" && tab ? "tab" : "side");
  const owner = target === "tab" && tab ? tab : side;
  const [draft, setDraft] = useState<ManualStatus>(owner.status ?? EMPTY);
  // Typed or recolored since the last load: a change from elsewhere then leaves the draft alone.
  const [dirty, setDirty] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const [name, setName] = useState(owner.title ?? "");
  const named = target === "side" ? props.sideName : (props.tabName ?? "");
  useEffect(() => {
    setDraft(owner.status ?? EMPTY);
    setDirty(false);
    setName(owner.title ?? "");
    input.current?.focus();
  }, [owner.id]);
  // Esc leaves the name as it was, even though the field loses focus on the way out.
  const cancelled = useRef(false);
  // A blank name goes back to the generated one.
  const saveName = () => {
    if (cancelled.current) return;
    const next = name.trim() || null;
    if (props.onRename && next !== (owner.title ?? null)) props.onRename(owner.id, next);
  };
  const current = JSON.stringify(owner.status ?? null);
  useEffect(() => {
    if (!dirty) setDraft(owner.status ?? EMPTY);
  }, [current]);

  const local = config.statuses.filter((s) => s.sideTabId === side.id);
  const global = config.statuses.filter((s) => s.sideTabId === null);
  const shown = statuses[owner.id];
  const auto = shown?.auto ?? "idle";
  const promotable = local.find((s) => s.id === draft.statusId);

  // A preset applies at once; typing or recoloring makes the draft a status of its own.
  const apply = (s: ManualStatus) => {
    setDraft(s);
    setDirty(false);
    void props.onSet(owner.id, s);
  };
  const edit = (patch: Partial<Look>) => {
    setDirty(true);
    setDraft((d) => ({ label: d.label, accent: d.accent, background: d.background, ...patch }));
  };
  const done = () => {
    saveName();
    const label = draft.label.trim();
    void props.onSet(owner.id, label ? { ...draft, label } : null);
    props.onClose();
  };
  const backToAuto = () => {
    setDraft(EMPTY);
    setDirty(false);
    void props.onSet(owner.id, null);
  };
  const resetAll = () => {
    setDraft(EMPTY);
    setDirty(false);
    for (const id of [side.id, ...side.tabs.map((t) => t.id)]) {
      const x = id === side.id ? side : side.tabs.find((t) => t.id === id);
      if (x?.status) void props.onSet(id, null);
    }
  };
  // The caller shows a failure; here it only stops the chain.
  const saveHere = () => void props.onSave(side.id, draft).then((saved) => apply(lookOf(saved)), () => undefined);

  return (
    <div className="sheet-back" onPointerDown={props.onClose}>
      <div
        className="ao-frame status-editor"
        role="dialog"
        tabIndex={-1}
        aria-labelledby="se-t"
        onPointerDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            cancelled.current = true;
            props.onClose();
          }
        }}
      >
        <div className="ao-frame-head">
          <i className="ao-i ao-i-dot" />
          <span id="se-t" className="ao-pixel">
            {props.onRename ? "Edit" : "Status"}
          </span>
          <span className="grow" />
          <span className="small ao-dim">
            {target === "side" ? "side tab" : "tab"} · {target === "side" ? props.sideName : props.tabName}
          </span>
        </div>
        <div className="ao-frame-body status-body">
          <div role="tablist" aria-label="Apply to" className="status-targets">
            <button type="button" className={`ao-btn${target === "side" ? "" : " ghost"}`} role="tab" aria-selected={target === "side"} onClick={() => setTarget("side")}>
              Side tab
            </button>
            {tab && (
              <button type="button" className={`ao-btn${target === "tab" ? "" : " ghost"}`} role="tab" aria-selected={target === "tab"} onClick={() => setTarget("tab")}>
                Tab: {props.tabName}
              </button>
            )}
          </div>
          {props.onRename && (
            <div className="edit-ident">
              <label className="ao-field grow">
                <span className="ao-pixel ao-dim">Name</span>
                <input
                  className="ao-input"
                  value={name}
                  placeholder={named}
                  aria-label={target === "side" ? "Side tab name" : "Tab name"}
                  onChange={(e) => setName(e.target.value)}
                  onBlur={saveName}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      done();
                    }
                  }}
                />
              </label>
              {props.onPickIcon && (
                <div className="ao-field">
                  <span className="ao-pixel ao-dim">Icon</span>
                  <button
                    type="button"
                    className="ao-btn ghost edit-icon"
                    aria-label="Change icon"
                    onClick={(e) => {
                      const r = e.currentTarget.getBoundingClientRect();
                      props.onPickIcon?.(owner.id, { x: r.left, y: r.bottom + 4 });
                    }}
                  >
                    <i className={`ao-i ao-i-${owner.icon ?? (target === "tab" ? "term" : side.tabs.length > 1 ? "group" : (side.tabs[0]?.icon ?? "term"))}`} style={owner.accent ? { color: `var(--neon-${owner.accent})` } : undefined} />
                  </button>
                </div>
              )}
            </div>
          )}
          {props.onAccent && (
            <div className="ao-field">
              <span className="ao-pixel ao-dim">{target === "side" ? "Row colour" : "Tab colour"}</span>
              <div className="neon-swatches" role="radiogroup" aria-label={target === "side" ? "Row colour" : "Tab colour"}>
                {NEONS.map((n) => (
                  <button
                    key={n}
                    type="button"
                    role="radio"
                    aria-checked={owner.accent === n}
                    aria-label={n}
                    title={n}
                    className={`neon-swatch${owner.accent === n ? " on" : ""}`}
                    style={{ "--sw": `var(--neon-${n})` } as React.CSSProperties}
                    onClick={() => props.onAccent?.(owner.id, n)}
                  />
                ))}
              </div>
            </div>
          )}
          <label className="ao-field">
            <span className="ao-pixel ao-dim">Status label</span>
            <input
              ref={input}
              className="ao-input"
              value={draft.label}
              maxLength={MAX_LABEL}
              placeholder={builtinLook(auto, config.builtins).label}
              aria-label="Status label"
              onChange={(e) => edit({ label: e.target.value })}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  done();
                }
              }}
            />
          </label>
          <div className="small ao-dim">
            auto would say: <StatusChip chip={chipOf({ auto, waiting: false, source: "auto" }, config.builtins)} />
            {owner.status && (
              <>
                {" · "}
                <button type="button" className="link" onClick={backToAuto}>
                  back to auto
                </button>
              </>
            )}
          </div>
          <div className="ao-field">
            <span className="ao-pixel ao-dim">This side tab</span>
            <div className="status-presets">
              {local.map((s) => (
                <button key={s.id} type="button" className="preset" onClick={() => apply(lookOf(s))}>
                  <StatusChip chip={{ ...s, set: true, need: false }} />
                </button>
              ))}
              <button type="button" className="ao-stat" disabled={!draft.label.trim()} onClick={saveHere}>
                + save here
              </button>
            </div>
          </div>
          <div className="ao-field">
            <span className="ao-pixel ao-dim">Global</span>
            <div className="status-presets">
              {!global.length && <span className="small ao-dim">No global statuses yet.</span>}
              {global.map((s) => (
                <button key={s.id} type="button" className="preset" onClick={() => apply(lookOf(s))}>
                  <StatusChip chip={{ ...s, set: true, need: false }} />
                </button>
              ))}
              {promotable && (
                <button type="button" className="ao-stat" onClick={() => void props.onMakeGlobal(promotable.id).catch(() => undefined)}>
                  ↑ make “{promotable.label}” global
                </button>
              )}
            </div>
            <div className="small ao-dim">
              Edit the global list in{" "}
              <button type="button" className="link" onClick={props.onSettings}>
                Settings › Statuses
              </button>
            </div>
          </div>
          <div className="ao-field">
            <span className="ao-pixel ao-dim">Accent</span>
            <Swatches kind="accent" value={draft.accent} onChange={(accent) => edit({ accent })} />
          </div>
          <div className="ao-field">
            <span className="ao-pixel ao-dim">Background</span>
            <Swatches kind="background" value={draft.background} onChange={(background) => edit({ background })} />
          </div>
          <div className="small ao-dim">Amber stays reserved: a session waiting on you shows its amber marker over any status you set.</div>
          {props.actions && (
            <div className="ao-field">
              <span className="ao-pixel ao-dim">{target === "side" ? "Side tab" : "Tab"}</span>
              <div className="edit-actions">
                {props.actions(target)
                  .filter((a): a is Exclude<MenuEntry, "-"> => a !== "-")
                  .map((a) => (
                    <ActionControl key={a.id} entry={a} onDone={props.onClose} />
                  ))}
              </div>
            </div>
          )}
        </div>
        <div className="ao-frame-foot">
          <button type="button" className="ao-btn ghost" onClick={resetAll}>
            Reset all to auto
          </button>
          <button type="button" className="ao-btn primary" onClick={done}>
            Done <span className="ao-key">↵</span>
          </button>
        </div>
      </div>
    </div>
  );
}

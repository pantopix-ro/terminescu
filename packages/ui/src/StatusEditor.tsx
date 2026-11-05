import { MAX_LABEL, type Look, type ManualStatus, type SavedStatus, type Shown, type SideTab, type StatusConfig, type Tab } from "@terminescu/host";
import { useEffect, useRef, useState } from "react";
import { StatusChip } from "./StatusChip";
import { builtinLook, chipOf } from "./status";
import { Swatches } from "./Swatches";

export interface StatusEditorProps {
  side: SideTab;
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
}

const EMPTY: ManualStatus = { label: "", accent: null, background: null };

const lookOf = (s: SavedStatus): ManualStatus => ({ statusId: s.id, label: s.label, accent: s.accent, background: s.background });

/** `⌘⇧S`: the status of the focused side tab or its active tab (design board "Status"). */
export function StatusEditor(props: StatusEditorProps) {
  const { side, tab, statuses, config } = props;
  const [target, setTarget] = useState<"side" | "tab">("side");
  const owner = target === "tab" && tab ? tab : side;
  const [draft, setDraft] = useState<ManualStatus>(owner.status ?? EMPTY);
  // Typed or recolored since the last load: a change from elsewhere then leaves the draft alone.
  const [dirty, setDirty] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    setDraft(owner.status ?? EMPTY);
    setDirty(false);
    input.current?.focus();
  }, [owner.id]);
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
            props.onClose();
          }
        }}
      >
        <div className="ao-frame-head">
          <i className="ao-i ao-i-dot" />
          <span id="se-t" className="ao-pixel">
            Status
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
          <label className="ao-field">
            <span className="ao-pixel ao-dim">Label</span>
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

import type { Pin, Variable } from "@terminescu/host";
import { useEffect, useRef, useState } from "react";

export type ContextMode = "pinned" | "auto";

export interface ContextPanelProps {
  mode: ContextMode;
  onMode(mode: ContextMode): void;
  /** The active side tab's name; the context belongs to it. */
  owner: string | undefined;
  variables?: Variable[];
  pins?: Pin[];
  /** Scrolls a transcript to the pin, opening one when needed (D12). */
  onOpenPin?(pin: Pin): void;
  onRemovePin?(pin: Pin): void;
}

const SECTIONS = ["Context", "Pins", "Prompts"] as const;
type Section = (typeof SECTIONS)[number];
const EMPTY: Record<Section, string> = {
  Context: "No variables yet.",
  Pins: "No pins yet.",
  Prompts: "No prompts yet.",
};

/** How long the pointer rests on the edge before the panel slides in. */
export const HOVER_MS = 150;

function Items({ section, variables = [], pins = [], onOpenPin, onRemovePin }: ContextPanelProps & { section: Section }) {
  if (section === "Context" && variables.length) {
    return (
      <ul className="ctx-list">
        {variables.map((v) => (
          <li key={v.id} className="ctx-var" data-var={v.name}>
            <span className="ctx-name">{v.name}</span>
            <span className="ctx-value small" title={v.value}>
              {v.value}
            </span>
          </li>
        ))}
      </ul>
    );
  }
  if (section === "Pins" && pins.length) {
    return (
      <ul className="ctx-list">
        {pins.map((p) => (
          <li key={p.id} className="ctx-pin" data-pin={p.id}>
            <button type="button" className="ctx-pin-open" title="Show in the transcript" onClick={() => onOpenPin?.(p)}>
              <i className="ao-i x1 ao-i-pin" />
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
  return <p className="small ao-dim ctx-empty">{EMPTY[section]}</p>;
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

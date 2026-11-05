import { useDroppable } from "@dnd-kit/core";
import type { AgentView, SideTab, Tab } from "@terminescu/host";
import { useState } from "react";
import { DOT, tabAgent, type Agents } from "./agents";

export interface SideTabListProps {
  sideTabs: SideTab[];
  activeId: string | undefined;
  activeTabId: string | undefined;
  onSelect(sideTabId: string): void;
  onSelectTab(sideTabId: string, tabId: string): void;
  onNew(): void;
  /** A tab is being dragged: show the drop targets (task 3.2). */
  dragging: boolean;
  /** Claude sessions by pane: a side tab with one waiting gets the amber marker (D34). */
  agents?: Agents;
}

/** A side tab's name: its own title, else its first tab's title, else "shell". */
export function sideTabName(s: SideTab): string {
  return s.title ?? s.tabs[0]?.title ?? "shell";
}

export const tabName = (t: Tab) => t.title ?? "shell";

/** `⌘1` to `⌘8` for the first tabs, `⌘9` for the last one, as the Window menu does. */
export function tabChord(i: number, count: number): string | undefined {
  if (i === count - 1 && count > 1) return "⌘9";
  return i < 8 ? `⌘${i + 1}` : undefined;
}

/** Drop ids: into a side tab, into the gap before side tab `i`, or after the last one. */
export const dropInto = (sideTabId: string) => `into:${sideTabId}`;
export const dropGap = (index: number) => `gap:${index}`;
export const DROP_END = "gap:end";

function Gap({ index, dragging, end }: { index: number; dragging: boolean; end?: boolean }) {
  const { setNodeRef, isOver } = useDroppable({ id: end ? DROP_END : dropGap(index), disabled: !dragging });
  if (end) {
    return (
      <div ref={setNodeRef} className="ao-drop side-end" hidden={!dragging}>
        New side tab
      </div>
    );
  }
  return (
    <div ref={setNodeRef} className="side-gap" data-gap={index}>
      {isOver && <div className="ao-drop-line" />}
    </div>
  );
}

function Row(props: {
  side: SideTab;
  index: number;
  active: boolean;
  expanded: boolean;
  activeTabId: string | undefined;
  dragging: boolean;
  agents: Agents;
  onSelect(): void;
  onSelectTab(tabId: string): void;
  onToggle(): void;
}) {
  const { side, active, expanded, dragging, agents } = props;
  const need = side.tabs.some((t) => tabAgent(t, agents)?.state === "waiting");
  const { setNodeRef, isOver } = useDroppable({ id: dropInto(side.id), disabled: !dragging || active });
  const group = side.tabs.length > 1;
  const open = group && (active || expanded);
  const style = active ? ({ "--st-bg": "var(--panel-2)" } as React.CSSProperties) : undefined;
  return (
    <>
      <div
        ref={setNodeRef}
        role="button"
        tabIndex={0}
        className={`ao-st${active ? " active" : ""}${isOver ? " drop-into" : ""}${need ? " need" : ""}`}
        style={style}
        data-side={side.id}
        aria-current={active ? "true" : undefined}
        onClick={props.onSelect}
        onKeyDown={(e) => e.key === "Enter" && props.onSelect()}
      >
        <i className={`ao-i ${group ? "ao-i-group" : side.tabs[0] && tabAgent(side.tabs[0], agents) ? "ao-i-agent" : "ao-i-term"}`} />
        <span className="t">{sideTabName(side)}</span>
        {group && !active ? (
          <button
            type="button"
            className="side-chev"
            aria-label={expanded ? "Collapse side tab" : "Expand side tab"}
            aria-expanded={expanded}
            onClick={(e) => {
              e.stopPropagation();
              props.onToggle();
            }}
          >
            <i className={`ao-i ${expanded ? "ao-i-chev" : "ao-i-chevr"}`} />
          </button>
        ) : (
          <span />
        )}
        <span className="m">
          {group && (
            <span className="ao-strip" aria-hidden="true">
              {side.tabs.map((t) => (
                <i key={t.id} className="ao-cell" />
              ))}
            </span>
          )}
          {side.tabs.length} {side.tabs.length === 1 ? "tab" : "tabs"}
        </span>
      </div>
      {open && (
        <div className="ao-st-kids" style={style}>
          {side.tabs.map((t, i) => (
            <div
              key={t.id}
              role="button"
              tabIndex={-1}
              className={`ao-st-kid${active && t.id === props.activeTabId ? " active" : ""}`}
              data-kid={t.id}
              onClick={() => props.onSelectTab(t.id)}
            >
              <KidIcon agent={tabAgent(t, agents)} />
              <span className="grow">{tabName(t)}</span>
              {active && <span>{tabChord(i, side.tabs.length)}</span>}
            </div>
          ))}
        </div>
      )}
    </>
  );
}

/**
 * The sidebar: one row per side tab. A side tab with more than one tab is a group and lists its
 * tabs under the row; the active group stays open, a chevron opens the others (D3).
 */
function KidIcon({ agent }: { agent: AgentView | undefined }) {
  if (!agent) return <i className="ao-i x1 ao-i-term" />;
  return (
    <>
      <i className="ao-i x1 ao-i-agent" aria-label="Claude" />
      <span className={`ao-dot ${DOT[agent.state]}`} title={agent.state} />
    </>
  );
}

export function SideTabList({
  sideTabs,
  activeId,
  activeTabId,
  onSelect,
  onSelectTab,
  onNew,
  dragging,
  agents = {},
}: SideTabListProps) {
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  return (
    <nav className="ao-side" aria-label="Side tabs">
      <div className="side-head">
        <span className="ao-pixel ao-dim">Side tabs</span>
        <span className="grow" />
        <button type="button" className="ao-btn ghost side-new" aria-label="New side tab" onClick={onNew}>
          <i className="ao-i ao-i-plus" />
        </button>
      </div>
      <div className="side-list">
        {sideTabs.map((s, i) => (
          <div key={s.id} className="side-item">
            <Gap index={i} dragging={dragging} />
            <Row
              side={s}
              index={i}
              active={s.id === activeId}
              expanded={!!expanded[s.id]}
              activeTabId={activeTabId}
              dragging={dragging}
              agents={agents}
              onSelect={() => onSelect(s.id)}
              onSelectTab={(t) => onSelectTab(s.id, t)}
              onToggle={() => setExpanded((m) => ({ ...m, [s.id]: !m[s.id] }))}
            />
          </div>
        ))}
        <Gap index={sideTabs.length} dragging={dragging} />
        <Gap index={sideTabs.length} dragging={dragging} end />
      </div>
      <div className="side-foot small ao-dim">
        <span className="ao-key">⌃⌘</span>
        <span className="ao-key">1</span>–<span className="ao-key">9</span> switch side tab
      </div>
    </nav>
  );
}

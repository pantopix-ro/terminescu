import { useDraggable, useDroppable } from "@dnd-kit/core";
import { leaf, panesOf, type AgentView, type Icon, type Shown, type SideTab, type Tab } from "@terminescu/host";
import { useState, type KeyboardEvent, type MouseEvent } from "react";
import { DOT, tabAgent, type Agents } from "./agents";
import { StatusChip } from "./StatusChip";
import { chipOf, rowStyle, type Builtins, type Chip, type SortMode } from "./status";

export interface SideTabListProps {
  sideTabs: SideTab[];
  activeId: string | undefined;
  activeTabId: string | undefined;
  onSelect(sideTabId: string): void;
  onSelectTab(sideTabId: string, tabId: string): void;
  onNew(): void;
  /** A tab is being dragged: show the drop targets (task 3.2). */
  dragging: boolean;
  /** The side tab being dragged onto another to merge (D5); rows become the only targets. */
  draggingSide?: string;
  /** Claude sessions by pane: a side tab with one waiting gets the amber marker (D34). */
  agents?: Agents;
  /** Shown statuses by tab and side tab id (D15). */
  statuses?: Readonly<Record<string, Shown>>;
  builtins?: Builtins;
  /** The order `sideTabs` comes in; drops between rows only make sense in manual order. */
  sort?: SortMode;
  /** Text panes' documents, for tab names. */
  docs?: DocMarks;
  /** A right click, Shift F10 or the menu key on a row (task 8.7), or a right click under the rows, at a window point. */
  onMenu?(at: { x: number; y: number }, target: MenuTarget): void;
  /** The side tab or tab id whose name is being edited in place. */
  renaming?: string;
  /** A new name, or null to go back to the generated one. */
  onRename?(id: string, title: string | null): void;
  onRenameEnd?(): void;
  /** A double click on a side tab's or a tab's name: edit it in place. */
  onRenameStart?(id: string): void;
  /** A click on a status chip, at a window point: pick that side tab's status, or its tab's. */
  onStatus?(sideTabId: string, tabId: string | undefined, at: { x: number; y: number }): void;
  /** A double click on a side tab's or a tab's icon, at a window point: pick another (task 8.16). */
  onIconPick?(id: string, at: { x: number; y: number }): void;
}

/** Where an icon picker opens: under the double-clicked icon. */
export const pickAt = (e: MouseEvent) => {
  const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
  return { x: r.left, y: r.bottom + 4 };
};

/**
 * A status chip that opens the quick status list. It stops the pointer and key events so the
 * row neither starts a drag nor selects itself.
 */
function ChipButton({ chip, label, onClick }: { chip: Chip; label: string; onClick?(at: { x: number; y: number }): void }) {
  if (!onClick) return <StatusChip chip={chip} />;
  return (
    <button
      type="button"
      className="chip-btn"
      aria-label={label}
      title="Set status"
      onPointerDown={(e) => e.stopPropagation()}
      onKeyDown={(e) => e.stopPropagation()}
      onClick={(e) => {
        e.stopPropagation();
        onClick(pickAt(e));
      }}
    >
      <StatusChip chip={chip} />
    </button>
  );
}

/** What a sidebar right click was on. */
export type MenuTarget = { kind: "side"; sideTabId: string } | { kind: "tab"; sideTabId: string; tabId: string } | { kind: "list" };

/** The point a menu opens at: the click, or under the row for the keyboard. */
function menuAt(e: MouseEvent | KeyboardEvent): { x: number; y: number } {
  if ("clientX" in e && e.type === "contextmenu" && (e.clientX || e.clientY)) return { x: e.clientX, y: e.clientY };
  const r = (e.currentTarget as Element).getBoundingClientRect();
  return { x: r.left + 16, y: r.bottom };
}

const menuKey = (e: KeyboardEvent) => e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey);

/** A name edited in place: Enter or leaving the field saves, Esc keeps the old one. */
export function RenameInput({ value, label, onDone, onCancel }: { value: string; label: string; onDone(v: string): void; onCancel(): void }) {
  const [text, setText] = useState(value);
  // Esc and Enter end the edit once; the blur that follows must not save again.
  const [ended, setEnded] = useState(false);
  const end = (save: boolean) => {
    if (ended) return;
    setEnded(true);
    if (save) onDone(text);
    else onCancel();
  };
  return (
    <input
      className="ao-input side-rename"
      aria-label={label}
      value={text}
      autoFocus
      spellCheck={false}
      onFocus={(e) => e.currentTarget.select()}
      onChange={(e) => setText(e.target.value)}
      // The row drags and selects on these; typing a name does neither.
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter") end(true);
        else if (e.key === "Escape") end(false);
      }}
      onBlur={() => end(true)}
    />
  );
}

/** A side tab's name: its own title, else its first tab's name, else "shell". */
export function sideTabName(s: SideTab, docs: DocMarks = {}): string {
  return s.title ?? (s.tabs[0] ? tabName(s.tabs[0], docs) : "shell");
}

/**
 * Text panes' documents by id: the title a tab shows, whether edits are unsaved and whether a
 * `terminescu edit --wait` call waits on it (D27).
 */
export type DocMarks = Readonly<Record<string, { title: string; dirty: boolean; request?: boolean }>>;

const docsOf = (t: Tab): string[] => panesOf(t.layout).flatMap((p) => leaf(t.layout, p)?.doc ?? []);

/** A tab's name: its own title, else the title of its first document, else "shell". */
export const tabName = (t: Tab, docs: DocMarks = {}) => t.title ?? docsOf(t).map((d) => docs[d]?.title).find(Boolean) ?? "shell";

/** A tab holds a document with unsaved edits. */
export const tabDirty = (t: Tab, docs: DocMarks = {}) => docsOf(t).some((d) => docs[d]?.dirty);

/** A tab holds a document a program waits on: it needs the person, as a waiting Claude does. */
export const tabRequest = (t: Tab, docs: DocMarks = {}) => docsOf(t).some((d) => docs[d]?.request);

/** `⌘1` to `⌘8` for the first tabs, `⌘9` for the last one, as the Window menu does. */
export function tabChord(i: number, count: number): string | undefined {
  if (i === count - 1 && count > 1) return "⌘9";
  return i < 8 ? `⌘${i + 1}` : undefined;
}

/** Drop ids: into a side tab, into the gap before side tab `i`, or after the last one. */
export const dropInto = (sideTabId: string) => `into:${sideTabId}`;
export const dropGap = (index: number) => `gap:${index}`;
/** Drag id of a side tab row, dropped onto another row to merge (D5). */
export const dragSide = (sideTabId: string) => `side:${sideTabId}`;
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
  draggingSide: string | undefined;
  agents: Agents;
  statuses: Readonly<Record<string, Shown>>;
  builtins: Builtins;
  docs: DocMarks;
  onSelect(): void;
  onSelectTab(tabId: string): void;
  onToggle(): void;
  onMenu?(at: { x: number; y: number }, target: MenuTarget): void;
  renaming?: string;
  onRename?(id: string, title: string | null): void;
  onRenameEnd?(): void;
  onRenameStart?(id: string): void;
  onStatus?(sideTabId: string, tabId: string | undefined, at: { x: number; y: number }): void;
  onIconPick?(id: string, at: { x: number; y: number }): void;
}) {
  const { side, active, expanded, dragging, agents, statuses, builtins, docs } = props;
  const shown = statuses[side.id];
  const need = shown?.waiting || side.tabs.some((t) => tabAgent(t, agents)?.state === "waiting" || tabRequest(t, docs));
  const chip = chipOf(shown, builtins);
  const { draggingSide } = props;
  // A tab drops into any other row; a side tab merges into any row but itself.
  const { setNodeRef, isOver } = useDroppable({
    id: dropInto(side.id),
    disabled: !dragging || (draggingSide ? draggingSide === side.id : active),
  });
  const drag = useDraggable({ id: dragSide(side.id) });
  const group = side.tabs.length > 1;
  const open = group && (active || expanded);
  // A status background tints the row; otherwise the active row takes the raised panel.
  const style = { ...(active ? { "--st-bg": "var(--panel-2)" } : {}), ...rowStyle(shown?.look, "st") } as React.CSSProperties;
  return (
    <>
      <div
        ref={(n) => {
          setNodeRef(n);
          drag.setNodeRef(n);
        }}
        {...drag.listeners}
        role="button"
        tabIndex={0}
        aria-roledescription="side tab"
        className={`ao-st${active ? " active" : ""}${isOver ? " drop-into" : ""}${need ? " need" : ""}${drag.isDragging ? " lifted" : ""}`}
        style={style}
        data-side={side.id}
        aria-current={active ? "true" : undefined}
        onClick={props.onSelect}
        onContextMenu={(e) => {
          if (!props.onMenu) return;
          e.preventDefault();
          props.onMenu(menuAt(e), { kind: "side", sideTabId: side.id });
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") props.onSelect();
          else if (menuKey(e) && props.onMenu) {
            e.preventDefault();
            props.onMenu(menuAt(e), { kind: "side", sideTabId: side.id });
          }
        }}
      >
        <i
          className={`ao-i ao-i-${side.icon ?? (group ? "group" : (side.tabs[0]?.icon ?? (side.tabs[0] && tabAgent(side.tabs[0], agents) ? "agent" : "term")))}`}
          title="Double-click to change the icon"
          onDoubleClick={(e) => {
            e.stopPropagation();
            props.onIconPick?.(side.id, pickAt(e));
          }}
        />
        {props.renaming === side.id ? (
          <RenameInput
            value={side.title ?? sideTabName(side, docs)}
            label="Side tab name"
            onDone={(v) => props.onRename?.(side.id, v.trim() || null)}
            onCancel={() => props.onRenameEnd?.()}
          />
        ) : (
          <span
            className="t"
            title="Double-click to rename"
            onDoubleClick={(e) => {
              e.stopPropagation();
              props.onRenameStart?.(side.id);
            }}
          >
            {sideTabName(side, docs)}
          </span>
        )}
        <span className="side-trail">
        <ChipButton
          chip={chip}
          label={`Status ${chip.label}: edit the status of ${sideTabName(side, docs)}`}
          onClick={props.onStatus && ((at) => props.onStatus!(side.id, undefined, at))}
        />
        {group && !active && (
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
        )}
        </span>
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
              onContextMenu={(e) => {
                if (!props.onMenu) return;
                e.preventDefault();
                props.onMenu(menuAt(e), { kind: "tab", sideTabId: side.id, tabId: t.id });
              }}
              onKeyDown={(e) => {
                if (menuKey(e) && props.onMenu) {
                  e.preventDefault();
                  props.onMenu(menuAt(e), { kind: "tab", sideTabId: side.id, tabId: t.id });
                }
              }}
            >
              <KidIcon
                agent={tabAgent(t, agents)}
                icon={t.icon}
                onDoubleClick={(e) => {
                  e.stopPropagation();
                  props.onIconPick?.(t.id, pickAt(e));
                }}
              />
              {props.renaming === t.id ? (
                <RenameInput
                  value={t.title ?? tabName(t, docs)}
                  label="Tab name"
                  onDone={(v) => props.onRename?.(t.id, v.trim() || null)}
                  onCancel={() => props.onRenameEnd?.()}
                />
              ) : (
                <span
                  className="grow"
                  title="Double-click to rename"
                  onDoubleClick={(e) => {
                    e.stopPropagation();
                    props.onRenameStart?.(t.id);
                  }}
                >
                  {tabName(t, docs)}
                </span>
              )}
              {statuses[t.id]?.look && (
                <ChipButton
                  chip={chipOf(statuses[t.id], builtins)}
                  label={`Status ${chipOf(statuses[t.id], builtins).label}: edit the status of ${tabName(t, docs)}`}
                  onClick={props.onStatus && ((at) => props.onStatus!(side.id, t.id, at))}
                />
              )}
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
/** A tab's icon: the one picked, else Claude's or a terminal's; Claude's state dot stays either way. */
export function KidIcon({ agent, icon, onDoubleClick }: { agent: AgentView | undefined; icon?: Icon; onDoubleClick?(e: MouseEvent): void }) {
  return (
    <>
      <i
        className={`ao-i x1 ao-i-${icon ?? (agent ? "agent" : "term")}`}
        aria-label={agent && !icon ? "Claude" : undefined}
        title={onDoubleClick && "Double-click to change the icon"}
        onDoubleClick={onDoubleClick}
      />
      {agent && <span className={`ao-dot ${DOT[agent.state]}`} title={agent.state} />}
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
  draggingSide,
  agents = {},
  statuses = {},
  builtins = {},
  sort = "manual",
  docs = {},
  onMenu,
  renaming,
  onRename,
  onRenameEnd,
  onRenameStart,
  onStatus,
  onIconPick,
}: SideTabListProps) {
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  return (
    <nav className="ao-side" aria-label="Side tabs">
      <div className="side-list">
        {sideTabs.map((s, i) => (
          <div key={s.id} className="side-item">
            <Gap index={i} dragging={dragging && !draggingSide && sort === "manual"} />
            <Row
              side={s}
              index={i}
              active={s.id === activeId}
              expanded={!!expanded[s.id]}
              activeTabId={activeTabId}
              dragging={dragging}
              draggingSide={draggingSide}
              agents={agents}
              statuses={statuses}
              builtins={builtins}
              docs={docs}
              onSelect={() => onSelect(s.id)}
              onSelectTab={(t) => onSelectTab(s.id, t)}
              onToggle={() => setExpanded((m) => ({ ...m, [s.id]: !m[s.id] }))}
              onMenu={onMenu}
              renaming={renaming}
              onRename={onRename}
              onRenameEnd={onRenameEnd}
              onRenameStart={onRenameStart}
              onStatus={onStatus}
              onIconPick={onIconPick}
            />
          </div>
        ))}
        <Gap index={sideTabs.length} dragging={dragging && !draggingSide && sort === "manual"} />
        <Gap index={sideTabs.length} dragging={dragging && !draggingSide} end />
        {/* The empty space under the last row: hovering it shows a + that adds a side tab. */}
        {!dragging && (
          <div
            className="side-fill"
            onDoubleClick={onNew}
            onContextMenu={(e) => {
              if (!onMenu) return;
              e.preventDefault();
              onMenu(menuAt(e), { kind: "list" });
            }}
          >
            <button type="button" className="side-add" aria-label="Add a side tab" title="New side tab (⌘N)" onClick={onNew}>
              <i className="ao-i ao-i-plus" />
            </button>
          </div>
        )}
      </div>
      <div className="side-foot small ao-dim">
        <span className="ao-key">⌃⌘</span>
        <span className="ao-key">1</span>–<span className="ao-key">9</span> switch side tab
      </div>
    </nav>
  );
}

import { useDraggable, useDroppable } from "@dnd-kit/core";
import { leaf, panesOf, type AgentView, type Icon, type Neon, type Shown, type SideTab, type Tab } from "@terminescu/host";
import { useEffect, useState, type KeyboardEvent, type MouseEvent } from "react";
import { actClass, type Activities, type Activity } from "./activity";
import { DOT, tabAgent, type Agents } from "./agents";
import { StatusChip } from "./StatusChip";
import { chipOf, rowStyle, type Builtins, type Chip, type SortMode } from "./status";
import { ShellPicker, usePicker } from "./ShellPicker";
import { Tip, useTip } from "./Tip";

type Tips = ReturnType<typeof useTip>;
type Picker = ReturnType<typeof usePicker>;

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
  /** Each tab's live subtitle by tab id: path, then program or Claude's title (task 8.30). */
  subtitles?: Readonly<Record<string, string>>;
  /** What runs in each tab, by tab id: icons blink, and bounce while Claude waits (task 8.54). */
  activity?: Activities;
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
  /** Only the rows' icons show; hovering one shows its name and status (task 8.23). */
  collapsed?: boolean;
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

/** A tab's name: its own title, else its first document's title, else its animal (task 8.30), else "shell". */
export const tabName = (t: Tab, docs: DocMarks = {}) => t.title ?? docsOf(t).map((d) => docs[d]?.title).find(Boolean) ?? t.name ?? "shell";

/** A name with its live subtitle, for a tooltip: `Otter · ~/work/api · npm`. */
export const withSub = (name: string, sub: string | undefined) => (sub ? `${name} · ${sub}` : name);

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
  subtitles: Readonly<Record<string, string>>;
  activity: Activities;
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
  collapsed: boolean;
  tips: Tips;
  picker: Picker;
}) {
  const { side, active, expanded, dragging, agents, statuses, builtins, docs, collapsed, tips, picker } = props;
  const name = sideTabName(side, docs);
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
  // A group stays open or closed as the person left it; it starts open (task 8.57).
  const open = group && expanded && !collapsed;
  // A status background tints the row; otherwise the active row takes the raised panel. The
  // row's own neon (task 8.25) paints the edge and the icon and washes 12% into that
  // background (the Wash board); the status keeps its chip.
  const look = rowStyle(shown?.look, "st");
  const ground = look["--st-bg"] ?? (active ? "var(--panel-2)" : "var(--panel)");
  // A row with one tab wears that tab's neon, so it matches the tab bar (task 8.51); a group
  // keeps its own and each tab under it wears its own.
  const accent = (!group && side.tabs[0]?.accent) || side.accent;
  const neon = accent && `var(--neon-${accent})`;
  const style = {
    ...look,
    "--st-bg": neon ? `color-mix(in srgb, ${neon} 12%, ${ground})` : ground,
    ...(neon ? { "--st-accent": neon } : {}),
  } as React.CSSProperties;
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
        aria-label={collapsed ? `${name}, ${chip.label}` : undefined}
        // A rest on the row lists its shells beside it (task 8.28); collapsed, every row does.
        onMouseEnter={(e) => {
          if ((collapsed || group) && !dragging && props.renaming !== side.id) picker.enter(side.id, e.currentTarget);
        }}
        onMouseLeave={() => {
          tips.hide();
          picker.leave();
        }}
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
          className={`ao-i ao-i-${side.icon ?? (group ? "group" : (side.tabs[0]?.icon ?? (side.tabs[0] && tabAgent(side.tabs[0], agents) ? "agent" : "term")))}${actClass(group ? undefined : props.activity[side.tabs[0]?.id ?? ""])}`}
          title={collapsed ? undefined : "Double-click to change the icon"}
          onDoubleClick={(e) => {
            e.stopPropagation();
            tips.hide();
            props.onIconPick?.(side.id, pickAt(e));
          }}
        />
        {props.renaming === side.id ? (
          <RenameInput
            value={side.title ?? name}
            label="Side tab name"
            onDone={(v) => props.onRename?.(side.id, v.trim() || null)}
            onCancel={() => props.onRenameEnd?.()}
          />
        ) : (
          <span
            className="t"
            title="Double-click to rename"
            // A name cut short shows whole in the custom tooltip, without the native one.
            onMouseEnter={(e) => (e.currentTarget.title = tips.expand(e.currentTarget, name) ? "" : "Double-click to rename")}
            onMouseLeave={tips.hide}
            onDoubleClick={(e) => {
              e.stopPropagation();
              tips.hide();
              props.onRenameStart?.(side.id);
            }}
          >
            {name}
          </span>
        )}
        <span className="side-trail">
        {group && (
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
          {/* One square per tab, in the tab's neon (task 8.26). A closed group's squares blink
              for their tabs, since its shell list is hidden (task 8.54). */}
          <span className="ao-strip" aria-hidden="true">
            {side.tabs.map((t) => (
              <i key={t.id} className={`ao-cell${group && !open ? actClass(props.activity[t.id]) : ""}`} data-cell={t.id} style={t.accent && { background: `var(--neon-${t.accent})` }} />
            ))}
          </span>
          {/* A one-tab row shows its tab's live subtitle here; a group's tabs show theirs below (task 8.30). */}
          <span className="grow st-sub">{!group && side.tabs[0] && props.subtitles[side.tabs[0].id]}</span>
          {/* The status sits under the name, so the name has the row's width (task 8.28). */}
          <ChipButton
            chip={chip}
            label={`Status ${chip.label}: edit the status of ${name}`}
            onClick={props.onStatus && ((at) => props.onStatus!(side.id, undefined, at))}
          />
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
              style={kidStyle(t, statuses[t.id])}
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
                accent={t.accent}
                act={props.activity[t.id]}
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
                  onMouseEnter={(e) => (e.currentTarget.title = tips.expand(e.currentTarget, withSub(tabName(t, docs), props.subtitles[t.id])) ? "" : "Double-click to rename")}
                  onMouseLeave={tips.hide}
                  onDoubleClick={(e) => {
                    e.stopPropagation();
                    tips.hide();
                    props.onRenameStart?.(t.id);
                  }}
                >
                  {props.activity[t.id] === "wait" && <AskMark />}
                  <span className="kid-n">{tabName(t, docs)}</span>
                  {props.subtitles[t.id] && <span className="kid-sub">{props.subtitles[t.id]}</span>}
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
 * tabs under the row. Each group starts open, and its chevron opens or closes it; switching
 * rows never closes one (D3, task 8.57).
 */
/** A tab's row under its group (task 8.51): its own status background, washed with its neon. */
function kidStyle(t: Tab, shown: Shown | undefined): React.CSSProperties {
  const ground = rowStyle(shown?.look, "st")["--st-bg"] ?? "var(--panel)";
  const bg = t.accent ? `color-mix(in srgb, var(--neon-${t.accent}) 12%, ${ground})` : ground;
  return { "--kid-bg": bg } as React.CSSProperties;
}

/** A tab's icon: the one picked, else Claude's or a terminal's; Claude's state dot stays either way. */
/** Claude's ✳ before a tab name while Claude waits for the person, as Claude Code titles it. */
export function AskMark() {
  return (
    <span className="ask-mark" aria-hidden="true">
      ✳
    </span>
  );
}

export function KidIcon({ agent, icon, accent, act, onDoubleClick }: { agent: AgentView | undefined; icon?: Icon; accent?: Neon; act?: Activity; onDoubleClick?(e: MouseEvent): void }) {
  return (
    <>
      <i
        className={`ao-i x1 ao-i-${icon ?? (agent ? "agent" : "term")}${actClass(act)}`}
        style={accent && { color: `var(--neon-${accent})` }}
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
  subtitles = {},
  activity = {},
  onMenu,
  renaming,
  onRename,
  onRenameEnd,
  onRenameStart,
  onStatus,
  onIconPick,
  collapsed = false,
}: SideTabListProps) {
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const tips = useTip();
  const picker = usePicker();
  const { hide } = tips;
  const { close } = picker;
  const hideAll = () => {
    hide();
    close();
  };
  // A drag, a rename, a scroll or a width change moves the rows from under the tooltip.
  useEffect(hideAll, [dragging, renaming, collapsed, sideTabs.length]);
  const picked = picker.at && sideTabs.find((s) => s.id === picker.at!.sideId);
  return (
    <nav className={`ao-side${collapsed ? " collapsed" : ""}`} aria-label="Side tabs">
      <div className="side-list" onScroll={hideAll}>
        {sideTabs.map((s, i) => (
          <div key={s.id} className={`side-item${s.id === activeId ? " active" : ""}`}>
            <Gap index={i} dragging={dragging && !draggingSide && sort === "manual"} />
            <Row
              side={s}
              index={i}
              active={s.id === activeId}
              expanded={expanded[s.id] ?? true}
              activeTabId={activeTabId}
              dragging={dragging}
              draggingSide={draggingSide}
              agents={agents}
              statuses={statuses}
              builtins={builtins}
              docs={docs}
              subtitles={subtitles}
              activity={activity}
              onSelect={() => onSelect(s.id)}
              onSelectTab={(t) => onSelectTab(s.id, t)}
              onToggle={() => setExpanded((m) => ({ ...m, [s.id]: !(m[s.id] ?? true) }))}
              onMenu={onMenu}
              renaming={renaming}
              onRename={onRename}
              onRenameEnd={onRenameEnd}
              onRenameStart={onRenameStart}
              onStatus={onStatus}
              onIconPick={onIconPick}
              collapsed={collapsed}
              tips={tips}
              picker={picker}
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
      <Tip tip={tips.tip} />
      {picker.at && picked && !dragging && (
        <ShellPicker at={picker.at} label={`Shells in ${sideTabName(picked, docs)}`} onEnter={picker.keep} onLeave={picker.leave}>
          <div className="px-pick-head">
            <span className="px-tip-name">{sideTabName(picked, docs)}</span>
            <span className="px-tip-status">{chipOf(statuses[picked.id], builtins).label}</span>
          </div>
          {picked.tabs.map((t) => (
            <button
              key={t.id}
              type="button"
              className={`px-pick-item${picked.id === activeId && t.id === activeTabId ? " active" : ""}`}
              data-pick={t.id}
              onClick={() => {
                close();
                onSelectTab(picked.id, t.id);
              }}
            >
              <KidIcon agent={tabAgent(t, agents)} icon={t.icon} accent={t.accent} act={activity[t.id]} />
              <span className="grow px-pick-text">
                <span className="px-pick-name">{tabName(t, docs)}</span>
                {subtitles[t.id] && <span className="px-pick-sub">{subtitles[t.id]}</span>}
              </span>
              {statuses[t.id]?.look && <StatusChip chip={chipOf(statuses[t.id], builtins)} />}
            </button>
          ))}
        </ShellPicker>
      )}
    </nav>
  );
}

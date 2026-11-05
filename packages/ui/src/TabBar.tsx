import { horizontalListSortingStrategy, SortableContext, useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { Shown, Tab } from "@terminescu/host";
import { DOT, tabAgent, type Agents } from "./agents";
import { StatusChip } from "./StatusChip";
import { chipOf, rowStyle, type Builtins } from "./status";
import { useEffect, useRef, useState } from "react";
import { tabDirty, tabName, tabRequest, type DocMarks } from "./SideTabList";

export interface TabBarProps {
  tabs: Tab[];
  activeId: string | undefined;
  onSelect(tabId: string): void;
  onClose(tabId: string): void;
  onNew(): void;
  /** `+ ▾ › Text`; with `⌥` held it splits the focused pane (D27). */
  onNewText?(beside: boolean): void;
  /** Claude sessions by pane: a tab with one shows the agent icon and its state (D34). */
  agents?: Agents;
  /** Shown statuses by tab id: a set status shows as a chip and colors the tab (D15). */
  statuses?: Readonly<Record<string, Shown>>;
  builtins?: Builtins;
  /** Text panes' documents: their title names the tab, unsaved edits show a dot (D27). */
  docs?: DocMarks;
}

function TabItem({
  tab,
  active,
  agents,
  shown,
  builtins,
  docs,
  onSelect,
  onClose,
}: {
  tab: Tab;
  active: boolean;
  agents: Agents;
  shown: Shown | undefined;
  builtins: Builtins;
  docs: DocMarks;
  onSelect(): void;
  onClose(): void;
}) {
  const agent = tabAgent(tab, agents);
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: tab.id });
  return (
    <div
      ref={setNodeRef}
      {...attributes}
      {...listeners}
      role="tab"
      tabIndex={0}
      aria-selected={active}
      data-tab={tab.id}
      data-agent={agent?.state}
      className={`ao-tab${active ? " active" : ""}${isDragging ? " placeholder" : ""}${shown?.waiting || agent?.state === "waiting" || tabRequest(tab, docs) ? " wait" : ""}`}
      style={{ transform: CSS.Translate.toString(transform), transition, ...rowStyle(shown?.look, "tab") }}
      onClick={onSelect}
      onKeyDown={(e) => e.key === "Enter" && onSelect()}
    >
      {agent && (
        <>
          <i className="ao-i x1 ao-i-agent" aria-label="Claude" />
          <span className={`ao-dot ${DOT[agent.state]}`} title={agent.state} />
        </>
      )}
      <span className="n">{tabName(tab, docs)}</span>
      {tabDirty(tab, docs) && <span className="tab-dirty" title="Unsaved changes" aria-label="Unsaved changes" />}
      {shown?.look && <StatusChip chip={chipOf(shown, builtins)} />}
      <button
        type="button"
        className="x"
        aria-label="Close tab"
        onPointerDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          e.stopPropagation();
          onClose();
        }}
      >
        ×
      </button>
    </div>
  );
}

/** The active side tab's tabs, across the top like browser tabs. Drag one to reorder it. */
/** `+ ▾`: a terminal tab or a text tab. */
function NewMenu({ onNew, onNewText }: { onNew(): void; onNewText(beside: boolean): void }) {
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => !box.current?.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("pointerdown", away);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("pointerdown", away);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);
  return (
    <span ref={box} className="tab-new-menu">
      <button type="button" className="ao-tab-new more" aria-label="Choose tab kind" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        ▾
      </button>
      {open && (
        <div role="menu" className="tab-menu ao-frame">
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setOpen(false);
              onNew();
            }}
          >
            <span className="grow">Terminal</span>
            <span className="small ao-dim">⌘T</span>
          </button>
          <button
            type="button"
            role="menuitem"
            title="Hold ⌥ to split the focused pane"
            onClick={(e) => {
              setOpen(false);
              onNewText(e.altKey);
            }}
          >
            <span className="grow">Text</span>
            <span className="small ao-dim">⌥⌘T</span>
          </button>
        </div>
      )}
    </span>
  );
}

export function TabBar({ tabs, activeId, onSelect, onClose, onNew, onNewText, agents = {}, statuses = {}, builtins = {}, docs = {} }: TabBarProps) {
  return (
    <div className="ao-tabs" role="tablist" data-tauri-drag-region>
      <SortableContext items={tabs.map((t) => t.id)} strategy={horizontalListSortingStrategy}>
        {tabs.map((t) => (
          <TabItem
            key={t.id}
            tab={t}
            active={t.id === activeId}
            agents={agents}
            shown={statuses[t.id]}
            builtins={builtins}
            docs={docs}
            onSelect={() => onSelect(t.id)}
            onClose={() => onClose(t.id)}
          />
        ))}
      </SortableContext>
      <button type="button" className="ao-tab-new" aria-label="New tab" onClick={onNew}>
        +
      </button>
      {onNewText && <NewMenu onNew={onNew} onNewText={onNewText} />}
      <span className="grow" data-tauri-drag-region />
      <span className="small ao-dim tabs-hint" data-tauri-drag-region>⌘D split · ⌘⇧D split down</span>
    </div>
  );
}

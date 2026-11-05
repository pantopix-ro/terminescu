import { horizontalListSortingStrategy, SortableContext, useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { Shown, Tab } from "@terminescu/host";
import { DOT, tabAgent, type Agents } from "./agents";
import { StatusChip } from "./StatusChip";
import { chipOf, rowStyle, type Builtins } from "./status";
import { tabDirty, tabName, type DocMarks } from "./SideTabList";

export interface TabBarProps {
  tabs: Tab[];
  activeId: string | undefined;
  onSelect(tabId: string): void;
  onClose(tabId: string): void;
  onNew(): void;
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
      className={`ao-tab${active ? " active" : ""}${isDragging ? " placeholder" : ""}${shown?.waiting || agent?.state === "waiting" ? " wait" : ""}`}
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
export function TabBar({ tabs, activeId, onSelect, onClose, onNew, agents = {}, statuses = {}, builtins = {}, docs = {} }: TabBarProps) {
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
      <span className="grow" data-tauri-drag-region />
      <span className="small ao-dim tabs-hint">⌘D split · ⌘⇧D split down</span>
    </div>
  );
}

import { horizontalListSortingStrategy, SortableContext, useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { Shown, Tab } from "@terminescu/host";
import { type Activities, type Activity } from "./activity";
import { tabAgent, type Agents } from "./agents";
import { StatusChip } from "./StatusChip";
import { chipOf, rowStyle, type Builtins } from "./status";
import { useEffect, useRef, useState } from "react";
import { NewMenu } from "./NewMenu";
import { Tip, useTip } from "./Tip";
import { AskMark, KidIcon, pickAt, RenameInput, tabDirty, tabName, tabRequest, type DocMarks } from "./SideTabList";

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
  /** Each tab's live subtitle by tab id: path, then program or Claude's title (task 8.30). */
  subtitles?: Readonly<Record<string, string>>;
  /** What runs in each tab, by tab id: its icon blinks, and bounces while Claude waits (task 8.54). */
  activity?: Activities;
  /** The tab whose name is being edited in place; a double click on a name starts it. */
  renaming?: string;
  onRenameStart?(tabId: string): void;
  /** A new name, or null to go back to the generated one. */
  onRename?(tabId: string, title: string | null): void;
  onRenameEnd?(): void;
  /** A double click on a tab's icon, at a window point: pick another (task 8.16). */
  onIconPick?(tabId: string, at: { x: number; y: number }): void;
  /** A right-click on a tab: open its edit dialog (task 8.34). */
  onEdit?(tabId: string): void;
}

function TabItem({
  tab,
  active,
  agents,
  shown,
  builtins,
  docs,
  sub,
  act,
  onSelect,
  onClose,
  renaming,
  onRenameStart,
  onRename,
  onRenameEnd,
  onIconPick,
  onEdit,
  tips,
}: Pick<TabBarProps, "onRenameStart" | "onRename" | "onRenameEnd" | "onIconPick" | "onEdit"> & {
  renaming: boolean;
  tab: Tab;
  active: boolean;
  agents: Agents;
  shown: Shown | undefined;
  builtins: Builtins;
  docs: DocMarks;
  sub: string | undefined;
  act: Activity | undefined;
  onSelect(): void;
  onClose(): void;
  tips: ReturnType<typeof useTip>;
}) {
  const agent = tabAgent(tab, agents);
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: tab.id });
  // The tab's neon washes 12% into its background, as its square under the row does (task 8.29).
  const look = rowStyle(shown?.look, "tab");
  const ground = active ? "var(--panel)" : (look["--tab-bg"] ?? "var(--panel-2)");
  const wash = tab.accent ? { "--tab-bg": `color-mix(in srgb, var(--neon-${tab.accent}) 12%, ${ground})` } : {};
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
      className={`ao-tab${active ? " active" : ""}${tab.accent ? " neon" : ""}${isDragging ? " placeholder" : ""}${shown?.waiting || agent?.state === "waiting" || tabRequest(tab, docs) ? " wait" : ""}`}
      style={{ transform: CSS.Translate.toString(transform), transition, ...look, ...wash } as React.CSSProperties}
      onClick={onSelect}
      // The whole name, the path and program and the status, as the sidebar rows show them (task 8.48).
      onMouseEnter={(e) => {
        if (renaming || isDragging) return;
        const chip = shown?.look || shown?.waiting ? chipOf(shown, builtins).label : undefined;
        tips.below(e.currentTarget, tabName(tab, docs), sub, chip);
      }}
      onMouseLeave={tips.hide}
      // Not onPointerDown: that one is the drag sensor's.
      onMouseDown={tips.hide}
      onContextMenu={(e) => {
        if (!onEdit) return;
        e.preventDefault();
        onEdit(tab.id);
      }}
      onKeyDown={(e) => e.key === "Enter" && onSelect()}
    >
      {/* Every tab shows its icon in its neon (task 8.29); one saved without an icon shows the default. */}
      <KidIcon
        agent={agent}
        icon={tab.icon}
        accent={tab.accent}
        act={act}
        onDoubleClick={(e) => {
          e.stopPropagation();
          onIconPick?.(tab.id, pickAt(e));
        }}
      />
      {renaming ? (
        <RenameInput
          value={tab.title ?? tabName(tab, docs)}
          label="Tab name"
          onDone={(v) => onRename?.(tab.id, v.trim() || null)}
          onCancel={() => onRenameEnd?.()}
        />
      ) : (
        <>
        {act === "wait" && <AskMark />}
        <span
          className="n"
          onDoubleClick={(e) => {
            e.stopPropagation();
            onRenameStart?.(tab.id);
          }}
        >
          {tabName(tab, docs)}
        </span>
        </>
      )}
      {!renaming && sub && <span className="tab-sub">{sub}</span>}
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

export function TabBar({ tabs, activeId, onSelect, onClose, onNew, onNewText, agents = {}, statuses = {}, builtins = {}, docs = {}, subtitles = {}, activity = {}, renaming, onRenameStart, onRename, onRenameEnd, onIconPick, onEdit }: TabBarProps) {
  const tips = useTip();
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
            sub={subtitles[t.id]}
            act={activity[t.id]}
            onSelect={() => onSelect(t.id)}
            onClose={() => onClose(t.id)}
            renaming={renaming === t.id}
            onRenameStart={onRenameStart}
            onRename={onRename}
            onRenameEnd={onRenameEnd}
            onIconPick={onIconPick}
            onEdit={onEdit}
            tips={tips}
          />
        ))}
      </SortableContext>
      <button type="button" className="ao-tab-new" aria-label="New tab" onClick={onNew}>
        +
      </button>
      {onNewText && <NewMenu onNew={onNew} onNewText={onNewText} />}
      <span className="grow" data-tauri-drag-region />
      <span className="small ao-dim tabs-hint" data-tauri-drag-region>⌘D split · ⌘⇧D split down</span>
      <Tip tip={tips.tip} />
    </div>
  );
}

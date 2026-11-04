import type { Tab } from "@terminescu/host";

export interface TabBarProps {
  tabs: Tab[];
  activeId: string | undefined;
  onSelect(tabId: string): void;
  onClose(tabId: string): void;
  onNew(): void;
}

/** The active side tab's tabs, across the top like browser tabs. */
export function TabBar({ tabs, activeId, onSelect, onClose, onNew }: TabBarProps) {
  return (
    <div className="ao-tabs" role="tablist" data-tauri-drag-region>
      {tabs.map((t) => (
        <div
          key={t.id}
          role="tab"
          tabIndex={0}
          aria-selected={t.id === activeId}
          className={`ao-tab${t.id === activeId ? " active" : ""}`}
          onClick={() => onSelect(t.id)}
          onKeyDown={(e) => e.key === "Enter" && onSelect(t.id)}
        >
          <span className="n">{t.title ?? "shell"}</span>
          <button
            type="button"
            className="x"
            aria-label="Close tab"
            onClick={(e) => {
              e.stopPropagation();
              onClose(t.id);
            }}
          >
            ×
          </button>
        </div>
      ))}
      <button type="button" className="ao-tab-new" aria-label="New tab" onClick={onNew}>
        +
      </button>
    </div>
  );
}

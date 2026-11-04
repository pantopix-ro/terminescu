import type { SideTab } from "@terminescu/host";

export interface SideTabListProps {
  sideTabs: SideTab[];
  activeId: string | undefined;
  onSelect(sideTabId: string): void;
  onNew(): void;
}

/** A side tab's name: its own title, else its first tab's title, else "shell". */
export function sideTabName(s: SideTab): string {
  return s.title ?? s.tabs[0]?.title ?? "shell";
}

/** The sidebar: one row per side tab, a block strip with one cell per tab for groups (D3). */
export function SideTabList({ sideTabs, activeId, onSelect, onNew }: SideTabListProps) {
  return (
    <nav className="ao-side" aria-label="Side tabs">
      {sideTabs.map((s) => (
        <button
          type="button"
          key={s.id}
          className={`ao-st${s.id === activeId ? " active" : ""}`}
          aria-current={s.id === activeId ? "true" : undefined}
          onClick={() => onSelect(s.id)}
        >
          <span />
          <span className="t">{sideTabName(s)}</span>
          <span />
          <span className="m">
            {s.tabs.length > 1 && (
              <span className="ao-strip" aria-hidden="true">
                {s.tabs.map((t) => (
                  <i key={t.id} className="ao-cell" />
                ))}
              </span>
            )}
            {s.tabs.length} {s.tabs.length === 1 ? "tab" : "tabs"}
          </span>
        </button>
      ))}
      <button type="button" className="ao-tab-new" aria-label="New side tab" onClick={onNew}>
        +
      </button>
    </nav>
  );
}

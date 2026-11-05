import type { Agents } from "./agents";

export interface StatusLineProps {
  /** Claude sessions by pane, in every window: the counts are app-wide, as the dock badge. */
  agents: Agents;
  sideTabs: number;
  tabs: number;
  /** The amber segment jumps to the session that has waited longest (`⌘⇧A`). */
  onJump(): void;
}

/** Waiting and running sessions across the app. */
export function attentionCounts(agents: Agents): { waiting: number; running: number } {
  let waiting = 0;
  let running = 0;
  for (const a of Object.values(agents)) {
    if (a.state === "waiting") waiting++;
    else if (a.state === "running") running++;
  }
  return { waiting, running };
}

/**
 * The bar across the bottom of a window (workbench-ui "Attention signals"). The amber segment
 * shows only while a session waits and is the only fill.
 */
export function StatusLine({ agents, sideTabs, tabs, onJump }: StatusLineProps) {
  const { waiting, running } = attentionCounts(agents);
  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
  return (
    <footer className="ao-status app-status">
      {waiting > 0 && (
        <button type="button" className="need" onClick={onJump} title="Jump to the session waiting longest (⌘⇧A)">
          <span className="ao-pixel">{waiting} needs you</span>
        </button>
      )}
      {running > 0 && (
        <span className="seg" style={{ color: "var(--blue)" }}>
          {running} running
        </span>
      )}
      <span className="seg">
        {plural(sideTabs, "side tab", "side tabs")} · {plural(tabs, "tab", "tabs")}
      </span>
      <span className="spacer" />
      <span className="seg">
        <span className="ao-key">⌘</span>
        <span className="ao-key">K</span>
      </span>
    </footer>
  );
}

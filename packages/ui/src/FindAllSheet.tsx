import { Command } from "cmdk";
import { useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { FIND_ALL_LIMIT, findInLines, readLines, type FindHit } from "./find-all";
import { terminalRows } from "./TerminalPane";

export interface FindAllSheetProps {
  /** The window's terminal panes, in tab order, with "Side › Tab" labels. */
  panes: { paneId: string; label: string }[];
  onPick(hit: FindHit): void;
  onClose(): void;
}

// Long lines show a window around the match, so the match stays in view.
const BEFORE = 40;
const WIDTH = 140;

function Excerpt({ hit, length }: { hit: FindHit; length: number }) {
  const from = Math.max(0, hit.at - BEFORE);
  const to = Math.min(hit.text.length, from + WIDTH);
  return (
    <span className="grow find-all-text">
      {from > 0 && "…"}
      {hit.text.slice(from, hit.at)}
      <mark>{hit.text.slice(hit.at, hit.at + length)}</mark>
      {hit.text.slice(hit.at + length, to)}
      {to < hit.text.length && "…"}
    </span>
  );
}

/** `⌘⇧F`: one query over every terminal in the window, its history included (task 8.46). */
export function FindAllSheet({ panes, onPick, onClose }: FindAllSheetProps) {
  const [query, setQuery] = useState("");
  const shown = useDeferredValue(query);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.focus(), []);
  // The buffers are read once as the sheet opens, so a key only searches strings; output
  // arriving later shows the next time the sheet opens.
  const ids = panes.map((p) => p.paneId).join(" ");
  const lines = useMemo(
    () =>
      readLines(
        ids.split(" ").flatMap((paneId) => {
          const rows = paneId ? terminalRows(paneId) : undefined;
          return rows ? [{ paneId, rows }] : [];
        }),
      ),
    [ids],
  );
  const found = useMemo(() => findInLines(lines, shown), [lines, shown]);
  const groups = panes.map((p) => ({ ...p, hits: found.hits.filter((h) => h.paneId === p.paneId) })).filter((g) => g.hits.length);
  return (
    <div className="palette-back" onPointerDown={onClose}>
      <Command
        className="palette ao-frame find-all"
        label="Find in all panes"
        loop
        shouldFilter={false}
        onPointerDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            onClose();
          }
        }}
      >
        <Command.Input ref={input} className="ao-input palette-input" placeholder="Find in all panes" aria-label="Find in all panes" value={query} onValueChange={setQuery} />
        <Command.List className="palette-list">
          {shown.trim() && !found.hits.length && <div className="palette-empty small ao-dim">No matches</div>}
          {groups.map((g) => (
            <Command.Group key={g.paneId} heading={g.label} className="palette-group">
              {g.hits.map((h) => (
                <Command.Item key={`${h.paneId}:${h.rows[0]}`} value={`${h.paneId}:${h.rows[0]}`} className="palette-item" onSelect={() => onPick(h)}>
                  <Excerpt hit={h} length={shown.length} />
                </Command.Item>
              ))}
            </Command.Group>
          ))}
          {found.more && <div className="palette-empty small ao-dim">Only the first {FIND_ALL_LIMIT} matches show.</div>}
        </Command.List>
        <div className="finder-foot small ao-dim">↵ show in its pane</div>
      </Command>
    </div>
  );
}

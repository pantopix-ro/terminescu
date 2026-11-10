import type { FileList } from "@terminescu/host";
import { Command } from "cmdk";
import { useEffect, useMemo, useRef, useState } from "react";
import { rankFiles } from "./files";
import { useChord } from "./chords";

export interface FileFinderProps {
  /** Files under the focused pane's cwd; undefined while they load. */
  list: FileList | undefined;
  /** `beside`: `⌥↵` splits the focused pane instead of adding a tab (D27). */
  onOpen(path: string, beside: boolean): void;
  /** The macOS Open dialog, also on `⌘O` again. */
  onBrowse(): void;
  onClose(): void;
}

const BROWSE = "browse…";

/** `⌘O`: files under the side tab's cwd, filtered as you type, then "Browse…" (D27). */
export function FileFinder({ list, onOpen, onBrowse, onClose }: FileFinderProps) {
  const openChord = useChord("open_file");
  const [query, setQuery] = useState("");
  const [value, setValue] = useState("");
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.focus(), []);
  const shown = useMemo(() => rankFiles(list?.files ?? [], query), [list, query]);
  const open = (rel: string, beside: boolean) => {
    onClose();
    if (rel === BROWSE) return onBrowse();
    onOpen(`${list!.root.replace(/\/$/, "")}/${rel}`, beside);
  };
  return (
    <div className="palette-back" onPointerDown={onClose}>
      <Command
        className="palette ao-frame"
        label="Open file"
        loop
        shouldFilter={false}
        value={value}
        onValueChange={setValue}
        onPointerDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            onClose();
          } else if (e.key === "Enter" && e.altKey && value) {
            e.preventDefault();
            open(value, true);
          }
        }}
      >
        <Command.Input ref={input} className="ao-input palette-input" placeholder="Search files by name" value={query} onValueChange={setQuery} />
        <Command.List className="palette-list">
          <Command.Group heading={list ? list.root : "Loading files…"} className="palette-group">
            {shown.map((f) => (
              <Command.Item key={f} value={f} className="palette-item" onSelect={() => open(f, false)}>
                <span className="grow">{f}</span>
              </Command.Item>
            ))}
          </Command.Group>
          {list?.truncated && <div className="palette-empty small ao-dim">Large folder: only part of it is listed.</div>}
          <Command.Item value={BROWSE} className="palette-item" onSelect={() => open(BROWSE, false)}>
            <span className="grow">Browse…</span>
            {openChord && <span className="small ao-dim">{openChord}</span>}
          </Command.Item>
        </Command.List>
        <div className="finder-foot small ao-dim">↵ open · ⌥↵ open beside</div>
      </Command>
    </div>
  );
}

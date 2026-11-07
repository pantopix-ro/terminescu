import { Command } from "cmdk";
import { useEffect, useRef } from "react";

export interface PaletteItem {
  id: string;
  label: string;
  group: string;
  hint?: string;
  run(): void;
}

export interface PaletteProps {
  items: PaletteItem[];
  onClose(): void;
}

/** `⌘P`: every app action and "Move tab to…", filtered as you type (task 3.2). */
export function Palette({ items, onClose }: PaletteProps) {
  const groups = [...new Set(items.map((i) => i.group))];
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.focus(), []);
  return (
    <div className="palette-back" onPointerDown={onClose}>
      <Command
        className="palette ao-frame"
        label="Command palette"
        loop
        onPointerDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            onClose();
          }
        }}
      >
        <Command.Input ref={input} className="ao-input palette-input" placeholder="Type an action or a side tab" />
        <Command.List className="palette-list">
          <Command.Empty className="palette-empty small ao-dim">No match.</Command.Empty>
          {groups.map((g) => (
            <Command.Group key={g} heading={g} className="palette-group">
              {items
                .filter((i) => i.group === g)
                .map((i) => (
                  <Command.Item
                    key={i.id}
                    value={`${i.group} ${i.label} ${i.id}`}
                    className="palette-item"
                    onSelect={() => {
                      onClose();
                      i.run();
                    }}
                  >
                    <span className="grow">{i.label}</span>
                    {i.hint && <span className="small ao-dim">{i.hint}</span>}
                  </Command.Item>
                ))}
            </Command.Group>
          ))}
        </Command.List>
      </Command>
    </div>
  );
}

import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import { finderOf, type FindOptions, type FindResults } from "./TerminalPane";

export interface SearchBarProps {
  paneId: string;
  /** Esc or the close button: the bar goes and the terminal takes the keys again. */
  onClose(): void;
}

/** What the app's `⌘F`, `⌘G` and `⌘⇧G` do to an open bar. */
export interface SearchBarHandle {
  focus(): void;
  next(back: boolean): void;
}

const OPTIONS: { key: keyof FindOptions; label: string; title: string }[] = [
  { key: "caseSensitive", label: "Aa", title: "Match case" },
  { key: "wholeWord", label: "W", title: "Whole word" },
  { key: "regex", label: ".*", title: "Regular expression" },
];

/**
 * Find in a terminal's scrollback (task 8.38), over the top right of its pane. Typing searches
 * as you go, `↵` and `⇧↵` step through the matches, and the scrollbar marks them.
 */
export const SearchBar = forwardRef<SearchBarHandle, SearchBarProps>(function SearchBar({ paneId, onClose }, ref) {
  const input = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [opts, setOpts] = useState<FindOptions>({ caseSensitive: false, wholeWord: false, regex: false });
  const [results, setResults] = useState<FindResults>();
  const [bad, setBad] = useState(false);

  const run = (back: boolean, incremental: boolean, q = query, o = opts) => {
    const finder = finderOf(paneId);
    if (!finder) return;
    if (o.regex && q) {
      try {
        new RegExp(q);
      } catch {
        setBad(true);
        finder.clear();
        setResults(undefined);
        return;
      }
    }
    setBad(false);
    if (!finder.find(q, o, back, incremental)) setResults(q ? { index: -1, count: 0 } : undefined);
  };

  useImperativeHandle(ref, () => ({
    focus: () => {
      input.current?.focus();
      input.current?.select();
    },
    next: (back) => run(back, false),
  }));

  useEffect(() => {
    input.current?.focus();
    const off = finderOf(paneId)?.listen(setResults);
    return () => {
      off?.();
      finderOf(paneId)?.clear();
    };
  }, [paneId]);

  const toggle = (key: keyof FindOptions) => {
    const o = { ...opts, [key]: !opts[key] };
    setOpts(o);
    run(false, true, query, o);
  };

  const count = !query ? "" : bad ? "Bad pattern" : !results || results.count === 0 ? "No matches" : results.index < 0 ? `${results.count}+` : `${results.index + 1} of ${results.count}`;
  return (
    <div className="find-bar ao-frame" role="search" onPointerDown={(e) => e.stopPropagation()}>
      <input
        ref={input}
        className="ao-input"
        aria-label="Find in terminal"
        placeholder="Find"
        value={query}
        spellCheck={false}
        onChange={(e) => {
          setQuery(e.target.value);
          run(false, true, e.target.value);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            run(e.shiftKey, false);
          } else if (e.key === "Escape") {
            e.preventDefault();
            onClose();
          }
        }}
      />
      <span className={`find-count small${bad || results?.count === 0 ? " none" : ""}`} aria-live="polite">
        {count}
      </span>
      {OPTIONS.map((o) => (
        <button key={o.key} type="button" className="find-opt" title={o.title} aria-label={o.title} aria-pressed={opts[o.key]} onClick={() => toggle(o.key)}>
          {o.label}
        </button>
      ))}
      <button type="button" className="find-opt" aria-label="Previous match" title="Previous match (⇧↵)" onClick={() => run(true, false)}>
        ↑
      </button>
      <button type="button" className="find-opt" aria-label="Next match" title="Next match (↵)" onClick={() => run(false, false)}>
        ↓
      </button>
      <button type="button" className="find-opt" aria-label="Close find" title="Close (Esc)" onClick={onClose}>
        ×
      </button>
    </div>
  );
});

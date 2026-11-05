import type { Entry, TranscriptChunk } from "@terminescu/host";
import { useEffect, useRef, useState } from "react";
import { anchor } from "./anchor";
import { EXTRACT_EVENT, PIN_EVENT } from "./events";

/** How often an open transcript reads what Claude added. */
const POLL_MS = 1000;

export type TextEntry = Extract<Entry, { kind: "user" | "assistant" }>;

/** A pin to scroll to and highlight; a new `nonce` reveals it again. */
export interface Reveal {
  entryId: string;
  exact: string;
  prefix: string | null;
  suffix: string | null;
  start: number | null;
  nonce: number;
}

export interface TranscriptPaneProps {
  paneId: string;
  session: string;
  read(offset: number): Promise<TranscriptChunk>;
  /** Pins a whole entry, or `[start, end)` of its text. */
  pin(entry: TextEntry, range?: [number, number]): Promise<unknown>;
  /** Opens the extract dialog for the text. */
  extract(value: string): void;
  /** Save as prompt, offered on the person's own messages (D14). */
  savePrompt?(text: string): void;
  reveal?: Reveal;
  onFocus(): void;
}

export { EXTRACT_EVENT, PIN_EVENT };

const shown = (e: Entry): e is TextEntry | Extract<Entry, { kind: "compaction" }> =>
  e.kind === "user" || e.kind === "assistant" || e.kind === "compaction";

/**
 * `[start, end)` of the selection inside `el`'s text, in UTF-16 units, or undefined when nothing
 * there is selected. A selection that runs past the message (a triple-click ends at the next
 * block) is clamped to it.
 */
function selectionIn(el: Element): [number, number] | undefined {
  const sel = window.getSelection();
  if (!sel || sel.isCollapsed || sel.rangeCount === 0) return undefined;
  const range = sel.getRangeAt(0).cloneRange();
  if (!range.intersectsNode(el)) return undefined;
  const full = document.createRange();
  full.selectNodeContents(el);
  if (range.compareBoundaryPoints(Range.START_TO_START, full) < 0) range.setStart(full.startContainer, full.startOffset);
  if (range.compareBoundaryPoints(Range.END_TO_END, full) > 0) range.setEnd(full.endContainer, full.endOffset);
  const before = document.createRange();
  before.selectNodeContents(el);
  before.setEnd(range.startContainer, range.startOffset);
  const start = before.toString().length;
  const end = start + range.toString().length;
  return end > start ? [start, end] : undefined;
}

/** The message holding the selection, with the selected range. */
function selected(root: HTMLElement): { entryId: string; range: [number, number] } | undefined {
  for (const el of root.querySelectorAll<HTMLElement>(".tr-msg[data-entry] .tr-text")) {
    const range = selectionIn(el);
    if (range) return { entryId: el.closest<HTMLElement>(".tr-msg")!.dataset.entry!, range };
  }
  return undefined;
}

/**
 * A read-only mirror of a Claude session: user and assistant messages with Pin and Extract per
 * message and per selection (task 4.4, D25). Tool calls, diffs and thinking come in P2.
 */
export function TranscriptPane({ paneId, session, read, pin, extract, savePrompt, reveal, onFocus }: TranscriptPaneProps) {
  const [entries, setEntries] = useState<Entry[]>([]);
  const [error, setError] = useState<string>();
  const [mark, setMark] = useState<{ entryId: string; range: [number, number] }>();
  const root = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const reader = useRef({ read });
  reader.current.read = read;

  useEffect(() => {
    let offset = 0;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    setEntries([]);
    const tick = async () => {
      try {
        const chunk = await reader.current.read(offset);
        if (stopped) return;
        offset = chunk.offset;
        setError(undefined);
        if (chunk.reset) {
          const seen = new Set<string>();
          setEntries(chunk.entries.filter((x) => !seen.has(x.id) && seen.add(x.id)));
        }
        else if (chunk.entries.length)
          setEntries((old) => {
            // A resumed or forked session can repeat a line's uuid; the first copy wins.
            const seen = new Set(old.map((x) => x.id));
            return [...old, ...chunk.entries.filter((x) => !seen.has(x.id) && seen.add(x.id))];
          });
      } catch (e) {
        if (!stopped) setError(String(e));
      }
      if (!stopped) timer = setTimeout(tick, POLL_MS);
    };
    void tick();
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, [session]);

  // New messages keep the view at the bottom when it was there.
  const atBottom = useRef(true);
  useEffect(() => {
    const el = list.current;
    if (el && atBottom.current && !mark) el.scrollTop = el.scrollHeight;
  }, [entries, mark]);

  const textOf = (id: string) => {
    const e = entries.find((x) => x.id === id);
    return e && (e.kind === "user" || e.kind === "assistant") ? e : undefined;
  };

  // A revealed pin scrolls to its message and highlights the quote once it loads, once per
  // nonce: later messages must not pull the view back to it.
  const revealed = useRef<number | undefined>(undefined);
  useEffect(() => {
    if (!reveal || revealed.current === reveal.nonce) return;
    const entry = textOf(reveal.entryId);
    if (!entry) return;
    revealed.current = reveal.nonce;
    const range = anchor(entry.text, reveal) ?? [0, entry.text.length];
    setMark({ entryId: entry.id, range });
    requestAnimationFrame(() =>
      list.current?.querySelector(`.tr-msg[data-entry="${CSS.escape(entry.id)}"]`)?.scrollIntoView({ block: "center" }),
    );
  }, [reveal?.nonce, reveal?.entryId, entries.length]);

  const startExtract = (value: string) => extract(value);
  const doPin = (entry: TextEntry, range?: [number, number]) => void pin(entry, range).catch((e: unknown) => setError(String(e)));

  // `⌘⇧P` and `⌘E` from the app act on the selection.
  useEffect(() => {
    const el = root.current!;
    const onPin = () => {
      const s = selected(el);
      const entry = s && textOf(s.entryId);
      if (s && entry) doPin(entry, s.range);
    };
    const onExtract = () => {
      const s = selected(el);
      const entry = s && textOf(s.entryId);
      if (s && entry) startExtract(entry.text.slice(...s.range));
    };
    el.addEventListener(PIN_EVENT, onPin);
    el.addEventListener(EXTRACT_EVENT, onExtract);
    return () => {
      el.removeEventListener(PIN_EVENT, onPin);
      el.removeEventListener(EXTRACT_EVENT, onExtract);
    };
  });

  const act = (entry: TextEntry, how: "pin" | "extract", msg: HTMLElement) => {
    const range = selectionIn(msg.querySelector(".tr-text")!);
    if (how === "pin") doPin(entry, range);
    else startExtract(range ? entry.text.slice(...range) : entry.text);
  };

  return (
    <div className="tr-pane" ref={root} data-transcript={paneId} tabIndex={-1} onFocus={onFocus}>
      <div
        className="tr-list"
        ref={list}
        // Scrolling by hand ends the highlight, and new messages follow again at the bottom.
        onWheel={() => mark && setMark(undefined)}
        onScroll={(e) => {
          const el = e.currentTarget;
          atBottom.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
        }}
      >
        {entries.filter(shown).map((e) =>
          e.kind === "compaction" ? (
            <div key={e.id} className="tr-compact small ao-dim">
              conversation compacted
            </div>
          ) : (
            <div key={e.id} className={`tr-msg tr-${e.kind}`} data-entry={e.id}>
              <div className="tr-head">
                <span className="tr-role">{e.kind === "user" ? "you" : "claude"}</span>
                <span className="grow" />
                <button
                  type="button"
                  className="ao-btn ghost tr-act"
                  onMouseDown={(ev) => ev.preventDefault()}
                  onClick={(ev) => act(e, "pin", ev.currentTarget.closest(".tr-msg")!)}
                >
                  Pin
                </button>
                <button
                  type="button"
                  className="ao-btn ghost tr-act"
                  onMouseDown={(ev) => ev.preventDefault()}
                  onClick={(ev) => act(e, "extract", ev.currentTarget.closest(".tr-msg")!)}
                >
                  Extract
                </button>
                {e.kind === "user" && savePrompt && (
                  <button type="button" className="ao-btn ghost tr-act" onMouseDown={(ev) => ev.preventDefault()} onClick={() => savePrompt(e.text)}>
                    Save as prompt
                  </button>
                )}
              </div>
              <div className="tr-text">
                {mark?.entryId === e.id ? (
                  <>
                    {e.text.slice(0, mark.range[0])}
                    <mark className="tr-mark">{e.text.slice(...mark.range)}</mark>
                    {e.text.slice(mark.range[1])}
                  </>
                ) : (
                  e.text
                )}
              </div>
            </div>
          ),
        )}
        {!entries.some(shown) && !error && <p className="small ao-dim tr-empty">No messages yet.</p>}
      </div>
      {error && (
        <p className="small app-error tr-error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}

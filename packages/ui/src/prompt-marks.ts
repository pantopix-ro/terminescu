import type { IDecoration, IMarker } from "@xterm/xterm";

/** One command as the shell integration marks it: prompt (A), output (C), end (D). */
interface Mark {
  prompt: IMarker;
  output?: IMarker;
  end?: IMarker;
  exit?: number;
  tick?: IDecoration;
}

/** The part of xterm.js the marks use, so @xterm/headless works in tests. */
export interface MarkTerminal {
  readonly buffer: { readonly active: { readonly type: string; readonly viewportY: number; readonly baseY: number; readonly cursorY: number } };
  readonly parser: { registerOscHandler(ident: number, cb: (data: string) => boolean): { dispose(): void } };
  registerMarker(offset?: number): IMarker | undefined;
  registerDecoration?(o: { marker: IMarker; overviewRulerOptions?: { color: string; position?: "left" | "center" | "right" | "full" } }): IDecoration | undefined;
  scrollToLine(line: number): void;
  scrollToBottom(): void;
  selectLines(start: number, end: number): void;
}

export interface PromptMarks {
  /** Scrolls to the prompt before (`back`) or after the one last jumped to; false when none. */
  jump(back: boolean): boolean;
  /** Selects the output of the command last jumped to, or of the last finished one. */
  selectOutput(): boolean;
  /** The live marks' prompt rows, oldest first, for tests. */
  rows(): number[];
  dispose(): void;
}

// The integration tags its own marks (src=terminescu); once one is seen, untagged ones from a
// nested shell's integration are ignored, as the daemon's ShellTracker does.
const OURS = "src=terminescu";

/**
 * Prompt marks from OSC 133 (task 8.45): each prompt gets a marker and a tick on the scrollbar,
 * red once its command fails. The daemon replays its marks in a reattach snapshot, so the same
 * handler rebuilds them. The alternate screen has no marks.
 */
export function attachPromptMarks(term: MarkTerminal, colors: { tick: string; failed: string }): PromptMarks {
  let marks: Mark[] = [];
  let tagged = false;
  // The prompt last jumped to and the viewport it left, so a scroll in between starts afresh.
  let nav: { mark: Mark; viewport: number } | undefined;

  const tick = (m: Mark, color: string) => {
    m.tick?.dispose();
    m.tick = term.registerDecoration?.({ marker: m.prompt, overviewRulerOptions: { color, position: "left" } });
  };
  const live = () => (marks = marks.filter((m) => !m.prompt.isDisposed));

  const handler = term.parser.registerOscHandler(133, (data) => {
    const parts = data.split(";");
    const mine = parts.includes(OURS);
    if (mine) tagged = true;
    else if (tagged) return false;
    if (term.buffer.active.type !== "normal") return false;
    const last = marks[marks.length - 1];
    switch (parts[0]) {
      case "A": {
        const prompt = term.registerMarker(0);
        if (!prompt) return false;
        // A prompt redrawn on the same row (a resize, `clear`) replaces the old mark.
        if (last && !last.output && last.prompt.line === prompt.line) {
          last.tick?.dispose();
          last.prompt.dispose();
          marks.pop();
        }
        const m: Mark = { prompt };
        // Marks trimmed off the top of the history go, so a long session holds only live ones.
        live().push(m);
        tick(m, colors.tick);
        return false;
      }
      case "C":
        if (last && !last.output) last.output = term.registerMarker(0);
        return false;
      case "D": {
        if (!last?.output || last.end) return false;
        last.end = term.registerMarker(0);
        const code = Number(parts[1]);
        if (Number.isInteger(code)) last.exit = code;
        if (last.exit) tick(last, colors.failed);
        return false;
      }
    }
    return false;
  });

  const cursorLine = () => term.buffer.active.baseY + term.buffer.active.cursorY;

  return {
    jump(back) {
      const list = live();
      const buf = term.buffer.active;
      const kept = nav && !nav.mark.prompt.isDisposed && buf.viewportY === nav.viewport ? nav.mark : undefined;
      // From a jump, step from that prompt; otherwise from the prompt being typed at, or the
      // top of a scrolled view. A view at the bottom has nothing after it.
      const current = list[list.length - 1];
      const typing = current && !current.output ? current.prompt.line : cursorLine();
      const from = kept ? kept.prompt.line : back ? typing : buf.viewportY;
      const atEnd = !kept && !back && buf.viewportY >= buf.baseY;
      const target = atEnd
        ? undefined
        : back
          ? list.filter((m) => m.prompt.line < from).pop()
          : list.find((m) => m.prompt.line > from && m.prompt.line < typing);
      if (!target) {
        nav = undefined;
        if (!back) term.scrollToBottom();
        return false;
      }
      term.scrollToLine(target.prompt.line);
      nav = { mark: target, viewport: term.buffer.active.viewportY };
      return true;
    },
    selectOutput() {
      const list = live();
      const buf = term.buffer.active;
      const kept = nav && !nav.mark.prompt.isDisposed && buf.viewportY === nav.viewport ? nav.mark : undefined;
      const done = (m: Mark) => m.output && m.end && !m.output.isDisposed && !m.end.isDisposed && m.end.line > m.output.line;
      const m = kept && done(kept) ? kept : list.filter(done).pop();
      if (!m) return false;
      term.selectLines(m.output!.line, m.end!.line - 1);
      return true;
    },
    rows: () => live().map((m) => m.prompt.line),
    dispose() {
      handler.dispose();
      for (const m of marks) m.tick?.dispose();
      marks = [];
    },
  };
}

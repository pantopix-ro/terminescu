//! The headless screen model of one pane: an alacritty `Term` fed with every byte the shell
//! writes (D31), plus the prompt marks the shell integration reports (D9).

use crate::osc::{MARK, OscScanner};
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::{Dimensions, Grid};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::Processor;
use std::collections::VecDeque;

/// Lines of history a pane keeps when its spawn names no amount. xterm.js keeps the same
/// amount in a window.
pub const SCROLLBACK: usize = 10_000;

/// The range a spawn's scrollback is clamped to.
pub const SCROLLBACK_RANGE: std::ops::RangeInclusive<usize> = 1_000..=50_000;

/// Marks kept per pane. History alone bounds them at one per row; this stops a flood of
/// prompts on a huge scrollback from growing the list past what a snapshot should carry.
const MAX_MARKS: usize = 20_000;

struct Size {
    cols: usize,
    rows: usize,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MarkKind {
    /// OSC 133;A, where a prompt starts.
    Prompt,
    /// OSC 133;C, where a command's output starts.
    Output,
}

/// A row the shell integration marked. The row is absolute: it counts scrolled rows, so it stays
/// put while the grid scrolls. A reflow that wraps rows into history leaves the count behind, so
/// it can go negative.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Mark {
    pub kind: MarkKind,
    pub row: i64,
    /// For a prompt, the exit code of the command that followed (OSC 133;D).
    pub exit: Option<i32>,
}

/// One of our own OSC 133 reports.
enum Report {
    Prompt,
    Output,
    End(Option<i32>),
}

impl Report {
    fn parse(payload: &[u8]) -> Option<Self> {
        let rest = std::str::from_utf8(payload).ok()?.strip_prefix("133;")?;
        let mut parts = rest.split(';');
        let kind = parts.next()?;
        let params: Vec<&str> = parts.collect();
        if !params.contains(&MARK) {
            return None;
        }
        match kind {
            "A" => Some(Self::Prompt),
            "C" => Some(Self::Output),
            "D" => Some(Self::End(params.first().and_then(|c| c.parse().ok()))),
            _ => None,
        }
    }
}

pub struct Screen {
    term: Term<VoidListener>,
    parser: Processor,
    /// Finds the end of each OSC 133 so `feed` can read the cursor at that exact byte.
    scanner: OscScanner,
    marks: VecDeque<Mark>,
}

impl Screen {
    pub fn new(cols: u16, rows: u16, scrollback: usize) -> Self {
        // Terminal queries (DA, DSR) go unanswered here: the attached xterm.js answers them, and
        // two answers would confuse the shell.
        let config = Config {
            scrolling_history: scrollback,
            kitty_keyboard: true,
            ..Config::default()
        };
        let size = Size {
            cols: cols.max(2) as usize,
            rows: rows.max(1) as usize,
        };
        Self {
            term: Term::new(config, &size, VoidListener),
            parser: Processor::new(),
            scanner: OscScanner::default(),
            marks: VecDeque::new(),
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        let mut reports = Vec::new();
        self.scanner.feed_at(bytes, |payload, end| {
            if let Some(r) = Report::parse(payload) {
                reports.push((end, r));
            }
        });
        let mut at = 0;
        for (end, report) in reports {
            self.parser.advance(&mut self.term, &bytes[at..end]);
            at = end;
            self.record(report);
        }
        self.parser.advance(&mut self.term, &bytes[at..]);
        self.prune();
    }

    /// Clears the history; with `keep_screen` false also moves the cursor's row to the top, as
    /// Terminal.app's "Clear to Start" and xterm.js `clear()` do. Only this model changes: the
    /// attached terminals clear themselves.
    pub fn clear(&mut self, keep_screen: bool) {
        // The alternate screen has no history, and the main screen waits for the program to leave.
        if self.term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        let n = self.term.grid().cursor.point.line.0;
        let seq = if keep_screen || n == 0 {
            "\x1b[3J".to_owned()
        } else {
            format!("\x1b[{n}S\x1b[3J\x1b[{n}A")
        };
        self.feed(seq.as_bytes());
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let size = Size {
            cols: cols.max(2) as usize,
            rows: rows.max(1) as usize,
        };
        if self.marks.is_empty() {
            self.term.resize(size);
            return;
        }
        // A width change reflows rows without scrolling them, so the absolute rows go stale.
        // Logical lines (a row and its wrapped continuations) keep their order and count, so
        // measure each mark in logical lines from the cursor's and rebuild the rows afterwards.
        let grid = self.main_grid();
        let before = Lines::of(grid);
        let cursor = before.index_of(grid.cursor.point.line.0);
        let offsets: Vec<(Mark, isize)> = self
            .marks
            .iter()
            .filter_map(|m| {
                let line = i32::try_from(m.row - before.scrolled).ok()?;
                Some((*m, before.index_of(line)? as isize - cursor? as isize))
            })
            .collect();
        self.term.resize(size);
        let grid = self.main_grid();
        let after = Lines::of(grid);
        let cursor = after.index_of(grid.cursor.point.line.0);
        let mut marks: Vec<Mark> = Vec::with_capacity(offsets.len());
        for (mark, offset) in offsets {
            let Some(start) = cursor
                .and_then(|c| usize::try_from(c as isize + offset).ok())
                .and_then(|i| after.starts.get(i))
            else {
                continue;
            };
            let row = after.scrolled + *start as i64;
            match marks.last_mut() {
                Some(last) if last.row == row && last.kind == mark.kind => {
                    last.exit = mark.exit.or(last.exit);
                }
                _ => marks.push(Mark { row, ..mark }),
            }
        }
        self.marks = marks.into();
        self.prune();
    }

    pub fn size(&self) -> (u16, u16) {
        (self.term.columns() as u16, self.term.screen_lines() as u16)
    }

    pub fn snapshot(&self) -> Vec<u8> {
        crate::snapshot::snapshot(&self.term, &self.marks, self.first_row())
    }

    pub fn term(&self) -> &Term<VoidListener> {
        &self.term
    }

    /// The marks in the order they were set.
    pub fn marks(&self) -> &VecDeque<Mark> {
        &self.marks
    }

    /// The absolute number of the oldest row the main screen still holds.
    pub fn first_row(&self) -> i64 {
        let grid = self.main_grid();
        grid.scrolled_rows() as i64 - grid.history_size() as i64
    }

    /// The main screen's grid, which is the inactive one while a program uses the alternate.
    fn main_grid(&self) -> &Grid<Cell> {
        if self.term.mode().contains(TermMode::ALT_SCREEN) {
            self.term.inactive_grid()
        } else {
            self.term.grid()
        }
    }

    fn record(&mut self, report: Report) {
        // Full-screen programs draw on the alternate screen; its rows are never scrollback.
        if self.term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        let grid = self.term.grid();
        let row = grid.scrolled_rows() as i64 + grid.cursor.point.line.0 as i64;
        let kind = match report {
            Report::Prompt => MarkKind::Prompt,
            Report::Output => MarkKind::Output,
            Report::End(code) => {
                let prompt = self
                    .marks
                    .iter_mut()
                    .rev()
                    .find(|m| m.kind == MarkKind::Prompt);
                if let (Some(code), Some(prompt)) = (code, prompt) {
                    prompt.exit = Some(code);
                }
                return;
            }
        };
        if self
            .marks
            .back()
            .is_some_and(|m| m.row == row && m.kind == kind)
        {
            return;
        }
        self.marks.push_back(Mark {
            kind,
            row,
            exit: None,
        });
        if self.marks.len() > MAX_MARKS {
            self.marks.pop_front();
        }
    }

    /// Drops marks whose row left the history, or went with a cleared history.
    fn prune(&mut self) {
        let first = self.first_row();
        while self.marks.front().is_some_and(|m| m.row < first) {
            self.marks.pop_front();
        }
    }
}

/// The logical lines of a grid: where each starts, as a line number (negative in history).
struct Lines {
    starts: Vec<i32>,
    /// The line number of the oldest row.
    top: i32,
    scrolled: i64,
}

impl Lines {
    fn of(grid: &Grid<Cell>) -> Self {
        let top = -(grid.history_size() as i32);
        let last = Column(grid.columns() - 1);
        let mut starts = Vec::new();
        let mut continues = false;
        for line in top..grid.screen_lines() as i32 {
            if !continues {
                starts.push(line);
            }
            continues = grid[Line(line)][last].flags.contains(Flags::WRAPLINE);
        }
        Self {
            starts,
            top,
            scrolled: grid.scrolled_rows() as i64,
        }
    }

    /// The index of the logical line that holds `line`, if it is in the grid.
    fn index_of(&self, line: i32) -> Option<usize> {
        (line >= self.top).then(|| self.starts.partition_point(|&s| s <= line) - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::MarkKind::{Output, Prompt};
    use super::*;

    const A: &str = "\x1b]133;A;src=terminescu\x07";
    const C: &str = "\x1b]133;C;src=terminescu\x07";

    fn d(code: i32) -> String {
        format!("\x1b]133;D;{code};src=terminescu\x07")
    }

    fn screen(cols: u16, rows: u16, scrollback: usize, input: &str) -> Screen {
        let mut s = Screen::new(cols, rows, scrollback);
        s.feed(input.as_bytes());
        s
    }

    /// Kind, row relative to the oldest row and exit of every mark.
    fn marks(s: &Screen) -> Vec<(MarkKind, i64, Option<i32>)> {
        let first = s.first_row();
        s.marks()
            .iter()
            .map(|m| (m.kind, m.row - first, m.exit))
            .collect()
    }

    /// The text of the row with the absolute number `row`.
    fn text(s: &Screen, row: i64) -> String {
        let grid = s.main_grid();
        let line = row - grid.scrolled_rows() as i64;
        let cells = &grid[Line(line as i32)];
        (0..grid.columns())
            .map(|c| cells[Column(c)].c)
            .collect::<String>()
            .trim_end()
            .to_owned()
    }

    fn session() -> String {
        format!(
            "{A}p1\r\n{C}out\r\n{}{A}p2\r\n{C}more\r\n{}{A}p3",
            d(1),
            d(0)
        )
    }

    #[test]
    fn marks_land_on_the_cursor_row() {
        let s = screen(20, 8, 100, &session());
        assert_eq!(
            marks(&s),
            [
                (Prompt, 0, Some(1)),
                (Output, 1, None),
                (Prompt, 2, Some(0)),
                (Output, 3, None),
                (Prompt, 4, None),
            ]
        );
        assert_eq!(text(&s, 0), "p1");
        assert_eq!(text(&s, 4), "p3");
    }

    #[test]
    fn marks_survive_scrolling_into_history() {
        let mut s = screen(20, 3, 100, &session());
        assert!(s.main_grid().history_size() > 0);
        for i in 0..10 {
            s.feed(format!("\r\nline {i}").as_bytes());
        }
        let m: Vec<_> = s.marks().iter().map(|m| (m.kind, m.row)).collect();
        assert_eq!(
            m,
            [
                (Prompt, 0),
                (Output, 1),
                (Prompt, 2),
                (Output, 3),
                (Prompt, 4)
            ]
        );
        assert_eq!(text(&s, 2), "p2");
        assert_eq!(text(&s, 4), "p3");
    }

    #[test]
    fn marks_beyond_the_history_are_pruned() {
        // Three screen lines and five lines of history: a row lives for eight rows in all.
        let mut s = screen(20, 3, 5, &session());
        assert_eq!(s.marks().len(), 5);
        for i in 0..6 {
            s.feed(format!("\r\nline {i}").as_bytes());
        }
        // 5 + 6 = 11 rows were written; rows 0..3 are gone, rows 3 and 4 remain.
        assert_eq!(s.first_row(), 3);
        let m: Vec<_> = s.marks().iter().map(|m| (m.kind, m.row)).collect();
        assert_eq!(m, [(Output, 3), (Prompt, 4)]);
        s.feed("\r\n".repeat(20).as_bytes());
        assert!(s.marks().is_empty());
    }

    #[test]
    fn a_report_split_across_chunks_uses_the_cursor_at_its_end() {
        let mut s = Screen::new(20, 5, 100);
        s.feed(b"abc\r\n\x1b]133;A;src=ter");
        s.feed(b"minescu\x07\r\nnext\x1b]133;A;src=terminescu\x07");
        assert_eq!(marks(&s), [(Prompt, 1, None), (Prompt, 2, None)]);
    }

    #[test]
    fn a_mark_is_set_once_per_row_and_kind() {
        let s = screen(20, 5, 100, &format!("{A}{A}$ {C}{C}"));
        assert_eq!(marks(&s), [(Prompt, 0, None), (Output, 0, None)]);
    }

    #[test]
    fn unmarked_reports_are_ignored() {
        let s = screen(
            20,
            5,
            100,
            "\x1b]133;A\x07\x1b]133;C;src=other\x07\x1b]133;D;1\x07x\x1b]7;file://h/\x07",
        );
        assert!(s.marks().is_empty());
    }

    #[test]
    fn the_alternate_screen_records_nothing() {
        let mut s = screen(20, 5, 100, &format!("{A}$ \r\n\x1b[?1049h{A}{C}{}", d(2)));
        assert_eq!(marks(&s), [(Prompt, 0, None)]);
        // A resize in the alternate screen still carries the main screen's marks.
        s.resize(10, 5);
        s.feed(b"\x1b[?1049l");
        assert_eq!(marks(&s), [(Prompt, 0, None)]);
    }

    #[test]
    fn a_width_change_moves_marks_with_their_logical_lines() {
        let long = "x".repeat(45);
        let mut s = screen(20, 6, 100, &format!("{A}ls\r\n{C}{long}\r\n{}{A}$ ", d(0)));
        // The long line takes three rows.
        assert_eq!(
            marks(&s),
            [(Prompt, 0, Some(0)), (Output, 1, None), (Prompt, 4, None)]
        );
        // Ten columns: five rows, and one row scrolls off the 6-line screen.
        s.resize(10, 6);
        assert_eq!(
            marks(&s),
            [(Prompt, 0, Some(0)), (Output, 1, None), (Prompt, 6, None)]
        );
        assert_eq!(text(&s, s.first_row() + 6), "$");
        assert_eq!(text(&s, s.first_row() + 1), "x".repeat(10));
        s.resize(40, 6);
        assert_eq!(
            marks(&s),
            [(Prompt, 0, Some(0)), (Output, 1, None), (Prompt, 3, None)]
        );
        assert_eq!(text(&s, s.first_row() + 3), "$");
        // New marks after the reflow land on the right row.
        s.feed(format!("\r\n{A}$ ").as_bytes());
        assert_eq!(marks(&s).last(), Some(&(Prompt, 4, None)));
    }

    #[test]
    fn a_height_change_keeps_the_rows() {
        let mut s = screen(20, 6, 100, &session());
        let before = marks(&s);
        s.resize(20, 3);
        s.resize(20, 9);
        assert_eq!(marks(&s), before);
        assert_eq!(text(&s, s.first_row() + 4), "p3");
    }

    #[test]
    fn the_snapshot_replays_the_marks_on_the_same_rows() {
        let mut input = session();
        for i in 0..12 {
            input.push_str(&format!("\r\nrow {i}"));
        }
        input.push_str(&format!(
            "\r\n{C}wrapped {}\r\n{}{A}$ ",
            "w".repeat(30),
            d(7)
        ));
        let a = screen(20, 5, 100, &input);
        assert!(a.main_grid().history_size() > 0);
        assert_eq!(a.marks().len(), 7);

        let snap = a.snapshot();
        let text_of = String::from_utf8_lossy(&snap).into_owned();
        assert_eq!(text_of.matches("\x1b]133;A;src=terminescu\x07").count(), 4);
        assert_eq!(text_of.matches("\x1b]133;C;src=terminescu\x07").count(), 3);
        assert!(text_of.contains(&d(1)) && text_of.contains(&d(0)) && text_of.contains(&d(7)));
        // The exit code of a prompt's command closes it before the next prompt's A.
        assert!(text_of.find(&d(1)).unwrap() < text_of.find("p2").unwrap());
        assert!(
            text_of.ends_with("")
                && text_of.find(&d(7)).unwrap() > text_of.find("wrapped").unwrap()
        );

        let mut b = Screen::new(20, 5, 100);
        b.feed(&snap);
        assert_eq!(marks(&a), marks(&b));
        for (x, y) in a.marks().iter().zip(b.marks()) {
            assert_eq!(text(&a, x.row), text(&b, y.row));
        }
        assert_eq!(a.snapshot(), b.snapshot());
    }

    #[test]
    fn a_mark_on_a_wrapped_continuation_row_replays_there() {
        let a = screen(10, 5, 100, &format!("0123456789t{A}ail\r\n{C}x"));
        assert_eq!(marks(&a), [(Prompt, 1, None), (Output, 2, None)]);
        let mut b = Screen::new(10, 5, 100);
        b.feed(&a.snapshot());
        assert_eq!(marks(&a), marks(&b));
    }

    #[test]
    fn clear_history_keeps_the_screen_and_the_marks_on_it() {
        let mut s = screen(20, 4, 100, &session());
        assert_eq!(s.main_grid().history_size(), 1);
        let screen_before: Vec<_> = (1..5).map(|r| text(&s, r)).collect();
        s.clear(true);
        assert_eq!(s.main_grid().history_size(), 0);
        assert_eq!(s.first_row(), 1);
        assert_eq!(
            s.marks().iter().map(|m| m.row).collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        assert_eq!(
            (1..5).map(|r| text(&s, r)).collect::<Vec<_>>(),
            screen_before
        );
    }

    #[test]
    fn clear_to_start_puts_the_cursor_row_on_top() {
        let mut s = screen(20, 4, 100, &session());
        let cursor_row = s.main_grid().scrolled_rows() as i64 + 3;
        assert_eq!(text(&s, cursor_row), "p3");
        s.clear(false);
        assert_eq!(s.main_grid().history_size(), 0);
        assert_eq!(s.main_grid().cursor.point.line.0, 0);
        assert_eq!(s.main_grid().cursor.point.column.0, 2);
        assert_eq!(text(&s, s.first_row()), "p3");
        assert_eq!(text(&s, s.first_row() + 1), "");
        // Only the cursor row's prompt is left, on the new top row.
        assert_eq!(marks(&s), [(Prompt, 0, None)]);
        // New output continues below it.
        s.feed(b"\r\nnext");
        assert_eq!(text(&s, s.first_row() + 1), "next");
    }

    #[test]
    fn clear_on_the_top_row_only_drops_history() {
        let mut s = screen(20, 4, 100, "a\r\nb\x1b[H");
        s.clear(false);
        assert_eq!(text(&s, s.first_row()), "a");
        assert_eq!(text(&s, s.first_row() + 1), "b");
    }
}

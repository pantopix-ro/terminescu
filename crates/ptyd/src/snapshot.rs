//! Turns a headless screen into bytes that rebuild it in a fresh terminal (D31).
//!
//! The output starts with RIS, so the receiving terminal (xterm.js in a window, or a fresh
//! alacritty `Term` in tests) ends up in the same state as the daemon's model: history, both
//! screens, cursor and pending wrap, saved cursor, scroll region, modes, kitty keyboard stacks,
//! cursor style, palette changes, charsets, pen and hyperlinks.
//!
//! Not restored: tab stops and the window title. Shells rarely change the first, and the
//! core tracks the second from OSC events.

use alacritty_terminal::grid::{Cursor, Dimensions, Grid};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::{Term, TermMode};
use alacritty_terminal::vte::ansi::{
    CharsetIndex, Color, CursorShape, CursorStyle, KeyboardModes, NamedColor, StandardCharset,
};
use std::fmt::Write;

/// Flags that change how a cell looks; the rest describe layout.
const STYLE: Flags = Flags::BOLD
    .union(Flags::DIM)
    .union(Flags::ITALIC)
    .union(Flags::INVERSE)
    .union(Flags::HIDDEN)
    .union(Flags::STRIKEOUT)
    .union(Flags::ALL_UNDERLINES);

#[derive(Clone, PartialEq, Eq)]
struct Pen {
    fg: Color,
    bg: Color,
    flags: Flags,
    underline: Option<Color>,
}

impl Default for Pen {
    fn default() -> Self {
        Self {
            fg: Color::Named(NamedColor::Foreground),
            bg: Color::Named(NamedColor::Background),
            flags: Flags::empty(),
            underline: None,
        }
    }
}

impl Pen {
    fn of(cell: &Cell) -> Self {
        Self {
            fg: cell.fg,
            bg: cell.bg,
            flags: cell.flags & STYLE,
            underline: cell.underline_color(),
        }
    }
}

struct Writer {
    out: String,
    pen: Pen,
    link: Option<(String, String)>,
}

impl Writer {
    fn new() -> Self {
        Self {
            out: String::with_capacity(64 * 1024),
            pen: Pen::default(),
            link: None,
        }
    }

    fn csi(&mut self, s: &str) {
        self.out.push_str("\x1b[");
        self.out.push_str(s);
    }

    fn cup(&mut self, line: usize, col: usize) {
        let _ = write!(self.out, "\x1b[{};{}H", line + 1, col + 1);
    }

    fn pen(&mut self, pen: &Pen) {
        if *pen == self.pen {
            return;
        }
        let mut s = String::from("0");
        let f = pen.flags;
        for (flag, code) in [
            (Flags::BOLD, "1"),
            (Flags::DIM, "2"),
            (Flags::ITALIC, "3"),
            (Flags::INVERSE, "7"),
            (Flags::HIDDEN, "8"),
            (Flags::STRIKEOUT, "9"),
            (Flags::UNDERLINE, "4"),
            (Flags::DOUBLE_UNDERLINE, "4:2"),
            (Flags::UNDERCURL, "4:3"),
            (Flags::DOTTED_UNDERLINE, "4:4"),
            (Flags::DASHED_UNDERLINE, "4:5"),
        ] {
            if f.contains(flag) {
                s.push(';');
                s.push_str(code);
            }
        }
        color(&mut s, pen.fg, 30, 90, 38);
        color(&mut s, pen.bg, 40, 100, 48);
        if let Some(u) = pen.underline {
            color(&mut s, u, 0, 0, 58);
        }
        s.push('m');
        self.csi(&s);
        self.pen = pen.clone();
    }

    fn link(&mut self, link: Option<(String, String)>) {
        if link == self.link {
            return;
        }
        match &link {
            Some((id, uri)) => {
                let _ = write!(self.out, "\x1b]8;id={id};{uri}\x1b\\");
            }
            None => self.out.push_str("\x1b]8;;\x1b\\"),
        }
        self.link = link;
    }

    fn cell(&mut self, cell: &Cell) {
        self.pen(&Pen::of(cell));
        self.link(
            cell.hyperlink()
                .map(|h| (h.id().to_owned(), h.uri().to_owned())),
        );
        self.out.push(cell.c);
        if let Some(zw) = cell.zerowidth() {
            self.out.extend(zw.iter());
        }
    }

    /// Default pen and no link, so erasing and scrolling fill rows with the default background.
    fn plain(&mut self) {
        self.pen(&Pen::default());
        self.link(None);
    }
}

/// Appends an SGR color. `base` and `bright` are the 8-color offsets; `ext` is 38, 48 or 58.
/// Underline color (58) has no short form, so it always uses the extended one.
fn color(s: &mut String, c: Color, base: u8, bright: u8, ext: u8) {
    match c {
        Color::Named(n) => {
            let i = n as usize;
            if ext != 58 && i < 8 {
                let _ = write!(s, ";{}", base as usize + i);
            } else if ext != 58 && i < 16 {
                let _ = write!(s, ";{}", bright as usize + i - 8);
            } else if i < 16 {
                let _ = write!(s, ";58:5:{i}");
            } else if ext != 58 {
                // Foreground, background and the dim variants all mean "default" in a cell.
                let _ = write!(s, ";{}", ext + 1);
            }
        }
        Color::Indexed(i) if ext == 58 => {
            let _ = write!(s, ";58:5:{i}");
        }
        Color::Indexed(i) => {
            let _ = write!(s, ";{ext};5;{i}");
        }
        Color::Spec(rgb) if ext == 58 => {
            let _ = write!(s, ";58:2::{}:{}:{}", rgb.r, rgb.g, rgb.b);
        }
        Color::Spec(rgb) => {
            let _ = write!(s, ";{ext};2;{};{};{}", rgb.r, rgb.g, rgb.b);
        }
    }
}

fn is_blank(cell: &Cell) -> bool {
    cell.c == ' '
        && cell.fg == Color::Named(NamedColor::Foreground)
        && cell.bg == Color::Named(NamedColor::Background)
        && (cell.flags & (STYLE | Flags::WIDE_CHAR)).is_empty()
        && cell.extra.is_none()
}

/// Writes every row of `grid`, history first, from the top-left corner of a cleared screen.
fn rows(w: &mut Writer, grid: &Grid<Cell>) {
    let cols = grid.columns();
    let top = -(grid.history_size() as i32);
    let bottom = grid.screen_lines() as i32 - 1;
    for line in top..=bottom {
        let row = &grid[Line(line)];
        let wrapped = row[Column(cols - 1)].flags.contains(Flags::WRAPLINE);
        // A wrapped row keeps its trailing blanks: the next row starts by wrapping (D31).
        let end = if wrapped {
            cols
        } else {
            (0..cols)
                .rev()
                .find(|&c| !is_blank(&row[Column(c)]))
                .map_or(0, |c| c + 1)
        };
        for col in 0..end {
            let cell = &row[Column(col)];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            w.cell(cell);
        }
        if line < bottom && !wrapped {
            w.plain();
            w.out.push_str("\r\n");
        }
    }
    w.plain();
}

fn charsets(w: &mut Writer, cursor: &Cursor<Cell>) {
    for (index, intro) in [
        (CharsetIndex::G0, '('),
        (CharsetIndex::G1, ')'),
        (CharsetIndex::G2, '*'),
        (CharsetIndex::G3, '+'),
    ] {
        let set = match cursor.charsets[index] {
            StandardCharset::Ascii => 'B',
            StandardCharset::SpecialCharacterAndLineDrawing => '0',
        };
        if set != 'B' {
            let _ = write!(w.out, "\x1b{intro}{set}");
        }
    }
}

fn pen_of(cursor: &Cursor<Cell>) -> (Pen, Option<(String, String)>) {
    let t = &cursor.template;
    (
        Pen::of(t),
        t.hyperlink()
            .map(|h| (h.id().to_owned(), h.uri().to_owned())),
    )
}

fn keyboard(w: &mut Writer, stack: &[KeyboardModes]) {
    for modes in stack {
        let _ = write!(w.out, "\x1b[>{}u", modes.bits());
    }
}

/// Private modes in `TermMode`, as DECSET numbers. `?1049` and kitty flags are handled apart.
const PRIVATE: [(TermMode, u16); 12] = [
    (TermMode::APP_CURSOR, 1),
    (TermMode::MOUSE_REPORT_CLICK, 1000),
    (TermMode::MOUSE_DRAG, 1002),
    (TermMode::MOUSE_MOTION, 1003),
    (TermMode::FOCUS_IN_OUT, 1004),
    (TermMode::UTF8_MOUSE, 1005),
    (TermMode::SGR_MOUSE, 1006),
    (TermMode::ALTERNATE_SCROLL, 1007),
    (TermMode::URGENCY_HINTS, 1042),
    (TermMode::BRACKETED_PASTE, 2004),
    (TermMode::SHOW_CURSOR, 25),
    (TermMode::LINE_WRAP, 7),
];

pub fn snapshot<T>(term: &Term<T>) -> Vec<u8> {
    let mut w = Writer::new();
    let alt = term.mode().contains(TermMode::ALT_SCREEN);
    let (main, alt_grid) = if alt {
        (term.inactive_grid(), Some(term.grid()))
    } else {
        (term.grid(), None)
    };

    w.out.push_str("\x1bc");

    let colors = term.colors();
    for i in 0..259 {
        if let Some(rgb) = colors[i] {
            let spec = format!("rgb:{:02x}/{:02x}/{:02x}", rgb.r, rgb.g, rgb.b);
            match i {
                0..=255 => {
                    let _ = write!(w.out, "\x1b]4;{i};{spec}\x1b\\");
                }
                256 => {
                    let _ = write!(w.out, "\x1b]10;{spec}\x1b\\");
                }
                257 => {
                    let _ = write!(w.out, "\x1b]11;{spec}\x1b\\");
                }
                _ => {
                    let _ = write!(w.out, "\x1b]12;{spec}\x1b\\");
                }
            }
        }
    }

    rows(&mut w, main);

    if let Some(grid) = alt_grid {
        // Leaving the alternate screen restores the main saved cursor, so park the main cursor
        // there before switching: `?1049h` saves it on both sides.
        let saved = &main.saved_cursor;
        w.cup(saved.point.line.0 as usize, saved.point.column.0);
        let (pen, link) = pen_of(saved);
        w.pen(&pen);
        w.link(link);
        keyboard(&mut w, term.inactive_keyboard_mode_stack());
        w.plain();
        w.out.push_str("\x1b[?1049h\x1b[H");
        rows(&mut w, grid);
    }
    let grid = alt_grid.unwrap_or(main);

    let region = term.scroll_region();
    if region.start.0 != 0 || region.end.0 as usize != grid.screen_lines() {
        let _ = write!(w.out, "\x1b[{};{}r", region.start.0 + 1, region.end.0);
    }

    // The saved cursor (DECSC) of the active screen. The main one is gone once `?1049h` ran.
    let saved = &grid.saved_cursor;
    charsets(&mut w, saved);
    w.cup(saved.point.line.0 as usize, saved.point.column.0);
    let (pen, link) = pen_of(saved);
    w.pen(&pen);
    w.link(link);
    w.out.push_str("\x1b7");
    w.plain();
    w.out.push_str("\x1b(B\x1b)B\x1b*B\x1b+B");

    let mode = *term.mode();
    let origin = mode.contains(TermMode::ORIGIN);
    if origin {
        w.out.push_str("\x1b[?6h");
    }

    // The cursor, and the pending wrap after a write to the last column.
    let cursor = &grid.cursor;
    let line = cursor.point.line.0 as usize - if origin { region.start.0 as usize } else { 0 };
    let col = cursor.point.column.0;
    w.cup(line, col);
    if cursor.input_needs_wrap {
        let cell = &grid[cursor.point.line][cursor.point.column];
        if cell.flags.contains(Flags::WIDE_CHAR_SPACER) && col > 0 {
            w.cup(line, col - 1);
            w.cell(&grid[cursor.point.line][Column(col - 1)]);
        } else {
            w.cell(cell);
        }
        w.plain();
    }

    for (flag, num) in PRIVATE {
        let on = mode.contains(flag);
        // Cursor shown and auto-wrap are on after RIS; everything else is off.
        let default_on = matches!(num, 25 | 7);
        if on != default_on {
            let _ = write!(w.out, "\x1b[?{}{}", num, if on { 'h' } else { 'l' });
        }
    }
    if mode.contains(TermMode::APP_KEYPAD) {
        w.out.push_str("\x1b=");
    }
    if mode.contains(TermMode::INSERT) {
        w.out.push_str("\x1b[4h");
    }
    if mode.contains(TermMode::LINE_FEED_NEW_LINE) {
        w.out.push_str("\x1b[20h");
    }
    keyboard(&mut w, term.keyboard_mode_stack());

    let style = term.cursor_style();
    if style != CursorStyle::default() {
        let n = match (style.shape, style.blinking) {
            (CursorShape::Block, true) => 1,
            (CursorShape::Block, false) => 2,
            (CursorShape::Underline, true) => 3,
            (CursorShape::Underline, false) => 4,
            (CursorShape::Beam, true) => 5,
            (CursorShape::Beam, false) => 6,
            _ => 0,
        };
        let _ = write!(w.out, "\x1b[{n} q");
    }

    charsets(&mut w, cursor);
    match term.active_charset() {
        CharsetIndex::G0 => {}
        CharsetIndex::G1 => w.out.push('\x0e'),
        CharsetIndex::G2 => w.out.push_str("\x1bn"),
        CharsetIndex::G3 => w.out.push_str("\x1bo"),
    }
    let (pen, link) = pen_of(cursor);
    w.pen(&pen);
    w.link(link);

    w.out.into_bytes()
}

#[cfg(test)]
mod tests {
    use crate::screen::Screen;
    use alacritty_terminal::grid::{Cursor, Dimensions, Grid};
    use alacritty_terminal::index::Line;
    use alacritty_terminal::term::TermMode;
    use alacritty_terminal::term::cell::Cell;

    fn rows_of(grid: &Grid<Cell>) -> Vec<Vec<Cell>> {
        let top = -(grid.history_size() as i32);
        (top..grid.screen_lines() as i32)
            .map(|l| grid[Line(l)].into_iter().cloned().collect())
            .collect()
    }

    fn cursor(c: &Cursor<Cell>, wrap: bool) -> String {
        format!(
            "{:?} {:?} {:?} wrap={}",
            c.point,
            c.template,
            c.charsets,
            wrap && c.input_needs_wrap
        )
    }

    /// Everything the snapshot promises to restore, as comparable values.
    fn state(s: &Screen) -> Vec<String> {
        let t = s.term();
        let alt = t.mode().contains(TermMode::ALT_SCREEN);
        let mut v = vec![
            format!("mode {:?}", t.mode()),
            format!("size {:?}", s.size()),
            format!("rows {:?}", rows_of(t.grid())),
            format!("cursor {}", cursor(&t.grid().cursor, true)),
            format!("saved {}", cursor(&t.grid().saved_cursor, false)),
            format!("region {:?}", t.scroll_region()),
            format!("kitty {:?}", t.keyboard_mode_stack()),
            format!("style {:?}", t.cursor_style()),
            format!("charset {:?}", t.active_charset()),
            format!(
                "colors {:?}",
                (0..259).map(|i| t.colors()[i]).collect::<Vec<_>>()
            ),
        ];
        if alt {
            let main = t.inactive_grid();
            v.push(format!("main rows {:?}", rows_of(main)));
            v.push(format!("main saved {:?}", main.saved_cursor.point));
            v.push(format!("main kitty {:?}", t.inactive_keyboard_mode_stack()));
        }
        v
    }

    fn screen(cols: u16, rows: u16, input: &[u8]) -> Screen {
        let mut s = Screen::new(cols, rows);
        s.feed(input);
        s
    }

    fn assert_round_trip(a: &Screen) {
        let (cols, rows) = a.size();
        let b = screen(cols, rows, &a.snapshot());
        let (sa, sb) = (state(a), state(&b));
        for (x, y) in sa.iter().zip(&sb) {
            assert_eq!(
                x,
                y,
                "\nsnapshot: {:?}",
                String::from_utf8_lossy(&a.snapshot())
            );
        }
        assert_eq!(sa.len(), sb.len());
        // The snapshot of the copy is the same bytes: nothing drifts across reattaches.
        assert_eq!(a.snapshot(), b.snapshot());
    }

    #[test]
    fn plain_history_and_prompt() {
        let mut input = Vec::new();
        for i in 1..=40 {
            input.extend_from_slice(format!("line {i}\r\n").as_bytes());
        }
        input.extend_from_slice(b"owner@mac ~ % ");
        assert_round_trip(&screen(20, 6, &input));
    }

    #[test]
    fn wrapped_row_keeps_trailing_blanks() {
        // "abc" and seven spaces fill row 0 exactly, then "de" wraps onto row 1.
        let a = screen(10, 4, b"abc       de\r\nnext");
        assert!(
            a.term().grid()[Line(0)][alacritty_terminal::index::Column(9)]
                .flags
                .contains(super::Flags::WRAPLINE)
        );
        assert_round_trip(&a);
    }

    #[test]
    fn scroll_region_and_origin_mode() {
        assert_round_trip(&screen(
            20,
            8,
            b"top\r\n\x1b[2;5r\x1b[?6h\x1b[2;3Hin region",
        ));
    }

    #[test]
    fn saved_cursor_with_pen_and_charset() {
        assert_round_trip(&screen(
            20,
            6,
            b"\x1b[3;5H\x1b[1;31m\x1b(0\x1b7\x1b(B\x1b[0m\x1b[1;1Hhome",
        ));
    }

    #[test]
    fn kitty_keyboard_stack() {
        assert_round_trip(&screen(20, 4, b"\x1b[>1u\x1b[>3u\x1b[>5u\x1b[<u"));
    }

    #[test]
    fn colors_styles_and_links() {
        let input = b"\x1b]4;1;rgb:12/34/56\x07\x1b]11;rgb:01/02/03\x07\x1b]12;rgb:ff/00/00\x07\
            \x1b[38;2;10;20;30;48;5;200mtrue\x1b[0m \x1b[1;2;3;7;9mmix\x1b[0m \
            \x1b[4:3;58:2::1:2:3mcurl\x1b[4:2;58;5;9mdouble\x1b[0m \
            \x1b]8;id=a1;https://example.invalid/PROJ-482\x1b\\link\x1b]8;;\x1b\\ \x1b[44m\x1b[K\x1b[0m\r\n\
            \x1b[91;102mbright\x1b[0m\x1b[3m";
        assert_round_trip(&screen(40, 5, input));
    }

    #[test]
    fn wide_and_combining_characters() {
        // The third wide character does not fit at the end of row 0 and wraps.
        assert_round_trip(&screen(5, 4, "ab日本語e\u{301}".as_bytes()));
    }

    #[test]
    fn pending_wrap_on_the_last_column() {
        let a = screen(6, 3, b"\r\n\r\n\x1b[1mabcdef");
        assert!(a.term().grid().cursor.input_needs_wrap);
        assert_round_trip(&a);
        let wide = screen(6, 3, "\r\n\r\nabcd日".as_bytes());
        assert!(wide.term().grid().cursor.input_needs_wrap);
        assert_round_trip(&wide);
    }

    #[test]
    fn modes_and_cursor_style() {
        assert_round_trip(&screen(20, 4, b"\x1b[?1h\x1b=\x1b[?2004h\x1b[?1000h\x1b[?1006h\x1b[?1004h\x1b[?25l\x1b[?7l\x1b[4h\x1b[20h\x1b[5 q\x1b)0\x0eabc"));
    }

    #[test]
    fn alternate_screen_survives_two_attaches() {
        let mut input = Vec::new();
        for i in 1..=30 {
            input.extend_from_slice(format!("$ seq {i}\r\n").as_bytes());
        }
        input.extend_from_slice(b"\x1b[3;4H\x1b[?1049h\x1b[>1u\x1b[2J\x1b[H\x1b[7m status \x1b[0m\r\n\x1b[32m> \x1b[0mask me\x1b[2;3r\x1b[2;4H");
        let a = screen(20, 6, &input);
        assert!(a.term().mode().contains(TermMode::ALT_SCREEN));
        assert_round_trip(&a);
        // A second attach sees the same screen: the snapshot reads the model without changing it.
        assert_eq!(a.snapshot(), a.snapshot());
        assert_round_trip(&a);
        // Leaving the alternate screen on the copy brings back the main screen and cursor.
        let mut b = screen(20, 6, &a.snapshot());
        let mut a2 = screen(20, 6, &input);
        b.feed(b"\x1b[?1049l");
        a2.feed(b"\x1b[?1049l");
        assert_eq!(state(&a2), state(&b));
    }

    #[test]
    fn resized_session() {
        let mut a = screen(30, 5, b"a long line that wraps around twice at least\r\n$ ");
        a.resize(12, 7);
        assert_round_trip(&a);
    }

    /// Replays the spike 0.1 stream (zsh, `seq 1 120`, one Claude answer, two resizes) and
    /// checks the snapshot against the committed file. `UPDATE_SNAPSHOT=1` rewrites the file.
    #[test]
    fn golden_claude_session() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reattach");
        let bytes = std::fs::read(dir.join("claude-session.bin")).unwrap();
        let events: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(dir.join("claude-session.events.json")).unwrap())
                .unwrap();
        let size = |e: &serde_json::Value| {
            (
                e["cols"].as_u64().unwrap() as u16,
                e["rows"].as_u64().unwrap() as u16,
            )
        };
        let (cols, rows) = size(&events[0]);
        let mut a = Screen::new(cols, rows);
        let mut at = 0;
        for e in &events[1..] {
            let offset = e["offset"].as_u64().unwrap() as usize;
            a.feed(&bytes[at..offset]);
            let (c, r) = size(e);
            a.resize(c, r);
            at = offset;
        }
        a.feed(&bytes[at..]);
        assert_round_trip(&a);

        let golden = dir.join("claude-session.snapshot.bin");
        let snap = a.snapshot();
        if std::env::var_os("UPDATE_SNAPSHOT").is_some() {
            std::fs::write(&golden, &snap).unwrap();
        }
        let want =
            std::fs::read(&golden).expect("missing snapshot file: run with UPDATE_SNAPSHOT=1");
        assert!(
            snap == want,
            "snapshot differs from {}; rerun with UPDATE_SNAPSHOT=1 after checking why",
            golden.display()
        );
    }
}

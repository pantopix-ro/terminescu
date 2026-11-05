//! The headless screen model of one pane: an alacritty `Term` fed with every byte the shell
//! writes (D31).

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;

/// Lines of history the model keeps. xterm.js keeps the same amount in a window.
pub const SCROLLBACK: usize = 10_000;

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

pub struct Screen {
    term: Term<VoidListener>,
    parser: Processor,
}

impl Screen {
    pub fn new(cols: u16, rows: u16) -> Self {
        // Terminal queries (DA, DSR) go unanswered here: the attached xterm.js answers them, and
        // two answers would confuse the shell.
        let config = Config {
            scrolling_history: SCROLLBACK,
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
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.term.resize(Size {
            cols: cols.max(2) as usize,
            rows: rows.max(1) as usize,
        });
    }

    pub fn size(&self) -> (u16, u16) {
        (self.term.columns() as u16, self.term.screen_lines() as u16)
    }

    pub fn snapshot(&self) -> Vec<u8> {
        crate::snapshot::snapshot(&self.term)
    }

    pub fn term(&self) -> &Term<VoidListener> {
        &self.term
    }
}

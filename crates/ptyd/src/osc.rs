//! Reads the shell integration's reports out of a pane's output (D9): OSC 133 for prompts and
//! exit codes, OSC 7 for the cwd and `OSC 633;E` for the command line, plus the window title a
//! program sets with OSC 0 or 2. The bytes still reach the screen and the clients untouched.

use terminescu_protocol::ShellState;

/// Longest OSC payload kept. A command line past this is cut, never split into garbage.
const MAX_PAYLOAD: usize = 8 * 1024;

/// Longest window title kept, in characters.
const MAX_TITLE: usize = 200;

/// Glyphs a program puts before its title to show it works or rests; Claude Code cycles ◐ and
/// ◑ every 960 ms while it works and shows ✳ at rest. Stripped, so the spinner never reads as a
/// new title.
const TITLE_GLYPHS: &[char] = &[
    '✳', '◐', '◑', '◒', '◓', '⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏',
];

#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum State {
    #[default]
    Ground,
    Esc,
    Osc,
    /// Inside an OSC, after an ESC that may start the `ESC \` terminator.
    OscEsc,
}

/// Finds OSC sequences across chunk boundaries.
#[derive(Default)]
pub struct OscScanner {
    state: State,
    payload: Vec<u8>,
    overflow: bool,
}

impl OscScanner {
    /// Calls `found` with the payload of every OSC that ends in `bytes`.
    pub fn feed(&mut self, bytes: &[u8], mut found: impl FnMut(&[u8])) {
        for &b in bytes {
            self.state = match (self.state, b) {
                (State::Ground, 0x1b) => State::Esc,
                (State::Ground, _) => State::Ground,
                (State::Esc, b']') => {
                    self.payload.clear();
                    self.overflow = false;
                    State::Osc
                }
                (State::Esc, 0x1b) => State::Esc,
                (State::Esc, _) => State::Ground,
                (State::Osc | State::OscEsc, 0x07) | (State::OscEsc, b'\\') => {
                    if !self.overflow {
                        found(&self.payload);
                    }
                    State::Ground
                }
                (State::Osc, 0x1b) => State::OscEsc,
                // CAN and SUB abort a sequence.
                (State::Osc, 0x18 | 0x1a) => State::Ground,
                (State::Osc, _) => {
                    self.push(b);
                    State::Osc
                }
                // An ESC that starts something else ends the OSC without a report.
                (State::OscEsc, b']') => {
                    self.payload.clear();
                    self.overflow = false;
                    State::Osc
                }
                (State::OscEsc, _) => State::Ground,
            };
        }
    }

    fn push(&mut self, b: u8) {
        if self.payload.len() < MAX_PAYLOAD {
            self.payload.push(b);
        } else {
            self.overflow = true;
        }
    }
}

/// Marks our own OSC 133 reports. Other integrations sourced from the person's rc files, such
/// as iTerm2's, emit OSC 133 too; once a pane sends a marked report its unmarked ones are
/// ignored, so the two never fight over the exit code.
pub const MARK: &str = "src=terminescu";

/// A pane's shell state and whether its reports carry our mark.
#[derive(Default)]
pub struct ShellTracker {
    pub state: ShellState,
    marked: bool,
}

impl ShellTracker {
    /// Applies one OSC payload. Returns whether the state changed.
    pub fn apply(&mut self, payload: &[u8]) -> bool {
        let Ok(text) = std::str::from_utf8(payload) else {
            return false;
        };
        if let Some(rest) = text.strip_prefix("133;") {
            if rest.split(';').any(|p| p == MARK) {
                self.marked = true;
            } else if self.marked {
                return false;
            }
        }
        apply(&mut self.state, text)
    }
}

fn apply(state: &mut ShellState, text: &str) -> bool {
    let before = state.clone();
    if let Some(raw) = text.strip_prefix("0;").or_else(|| text.strip_prefix("2;")) {
        // A title says nothing about the integration, so it leaves `integrated` alone.
        state.title = title_of(raw);
        return *state != before;
    }
    if let Some(rest) = text.strip_prefix("133;") {
        let mut parts = rest.split(';');
        match parts.next() {
            Some("A") => state.running = false,
            Some("C") => {
                state.running = true;
                state.last_exit = None;
                state.started += 1;
                state.title = None;
            }
            Some("D") => {
                state.running = false;
                state.title = None;
                let Some(code) = parts.next().and_then(|c| c.parse().ok()) else {
                    return false;
                };
                state.last_exit = Some(code);
                state.ended += 1;
            }
            _ => return false,
        }
    } else if let Some(line) = text.strip_prefix("633;E;") {
        state.command = Some(unescape_command(line));
    } else if let Some(url) = text.strip_prefix("7;") {
        match cwd_of(url) {
            Some(cwd) => state.cwd = Some(cwd),
            None => return false,
        }
    } else {
        return false;
    }
    state.integrated = true;
    *state != before
}

/// A title without its leading glyphs and control characters, cut to [`MAX_TITLE`]; none
/// when nothing is left.
fn title_of(raw: &str) -> Option<String> {
    let clean: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim_start_matches(|c: char| TITLE_GLYPHS.contains(&c) || c.is_whitespace())
        .trim_end()
        .chars()
        .take(MAX_TITLE)
        .collect();
    (!clean.is_empty()).then_some(clean)
}

/// `\\` is a backslash and `\xHH` a byte; the integration escapes `;` and control characters
/// that way, as VS Code's `633;E` does.
fn unescape_command(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            if bytes.get(i + 1) == Some(&b'\\') {
                out.push(b'\\');
                i += 2;
                continue;
            }
            if bytes.get(i + 1) == Some(&b'x')
                && let Some(b) = bytes
                    .get(i + 2..i + 4)
                    .and_then(|h| std::str::from_utf8(h).ok())
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(b);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The path of a `file://host/path` URL, percent-decoded.
fn cwd_of(url: &str) -> Option<String> {
    let rest = url.strip_prefix("file://")?;
    let path = &rest[rest.find('/')?..];
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(b) = path
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(chunks: &[&[u8]]) -> Vec<String> {
        let mut s = OscScanner::default();
        let mut out = Vec::new();
        for c in chunks {
            s.feed(c, |p| out.push(String::from_utf8_lossy(p).into_owned()));
        }
        out
    }

    #[test]
    fn finds_sequences_across_chunks_and_terminators() {
        assert_eq!(
            scan(&[
                b"a\x1b]133;A\x07b\x1b]7;file://h/tmp\x1b",
                b"\\c\x1b]13",
                b"3;D;1\x07"
            ]),
            ["133;A", "7;file://h/tmp", "133;D;1"]
        );
        // A CSI is not an OSC; an aborted OSC reports nothing.
        assert_eq!(
            scan(&[b"\x1b[31mred\x1b]133;A\x18\x1b]133;C\x07"]),
            ["133;C"]
        );
        let long = [
            b"\x1b]633;E;".as_slice(),
            &vec![b'x'; MAX_PAYLOAD],
            b"\x07\x1b]133;C\x07",
        ]
        .concat();
        assert_eq!(scan(&[&long]), ["133;C"], "an overlong payload is dropped");
    }

    #[test]
    fn a_command_cycle_updates_the_state() {
        let mut t = ShellTracker::default();
        assert!(t.apply(b"7;file://mac.local/Users/a%20b/50%25"));
        assert_eq!(t.state.cwd.as_deref(), Some("/Users/a b/50%"));
        assert!(t.apply(br"633;E;cd /tmp\x3b false \\ok"));
        assert_eq!(t.state.command.as_deref(), Some(r"cd /tmp; false \ok"));
        assert!(t.apply(b"133;C;src=terminescu"));
        assert!(t.state.running && t.state.integrated);
        assert!(t.apply(b"133;D;1;src=terminescu"));
        assert_eq!((t.state.running, t.state.last_exit), (false, Some(1)));
        assert!(
            !t.apply(b"133;A;src=terminescu"),
            "a prompt after the end changes nothing"
        );
        assert!(!t.apply(b"7;not a url"));
        assert!(
            !t.apply(b"133;D;src=terminescu"),
            "an end without a code is ignored"
        );
    }

    #[test]
    fn another_integration_is_ignored_once_ours_reports() {
        let mut t = ShellTracker::default();
        // Before ours speaks, another integration's reports still help.
        assert!(t.apply(b"133;D;0"));
        assert_eq!(t.state.last_exit, Some(0));
        assert!(t.apply(b"133;D;1;src=terminescu"));
        assert!(!t.apply(b"133;D;0"), "iTerm2's report after an empty line");
        assert!(!t.apply(b"133;C;"));
        assert_eq!((t.state.running, t.state.last_exit), (false, Some(1)));
    }

    #[test]
    fn a_title_loses_its_spinner_and_clears_when_a_command_starts_or_ends() {
        let mut t = ShellTracker::default();
        assert!(t.apply("0;✳ Claude Code".as_bytes()));
        assert_eq!(t.state.title.as_deref(), Some("Claude Code"));
        assert!(t.apply("2;◐ Fix the export".as_bytes()));
        assert_eq!(t.state.title.as_deref(), Some("Fix the export"));
        // The spinner turns: the title stays, so nothing changed.
        assert!(!t.apply("2;◑ Fix the export".as_bytes()));
        assert!(!t.state.integrated, "a title is no shell integration");
        // A blank title clears it.
        assert!(t.apply(b"0;  "));
        assert_eq!(t.state.title, None);
        assert!(t.apply("0;✳ Plan".as_bytes()));
        assert!(t.apply(b"133;C;src=terminescu"));
        assert_eq!(t.state.title, None);
        assert!(t.apply(b"0;vim"));
        assert!(t.apply(b"133;D;0;src=terminescu"));
        assert_eq!(t.state.title, None);
        let long = format!("0;{}", "x".repeat(500));
        assert!(t.apply(long.as_bytes()));
        assert_eq!(
            t.state.title.as_ref().map(|s| s.chars().count()),
            Some(MAX_TITLE)
        );
    }
}

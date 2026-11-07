//! App settings that are not statuses, commands or attention signals (D25): the defaults an
//! extracted variable starts with, the opt-in editor setting for the shell (D27) and the zoom
//! of every window (task 8.8), the sidebar's width and whether it shows only icons (task 8.23),
//! the terminal's font and spacing (task 8.36) and its cursor, history, keys, bell, clipboard
//! and contrast (task 8.42). Kept as one JSON value in `app_state`.

use crate::context::Mode;
use crate::db::{Db, Result};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

const KEY: &str = "app.settings";
/// Longest TTL a default may hold: 30 days.
pub const MAX_TTL_SECS: u64 = 30 * 86_400;
/// The zoom steps `⌘+` and `⌘-` walk, in percent, as a browser has them.
pub const ZOOM_STEPS: [u16; 11] = [50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200];
/// The sidebar's width in CSS pixels: the default and the range a drag may set.
pub const SIDEBAR_WIDTH: u16 = 272;
pub const SIDEBAR_MIN: u16 = 180;
pub const SIDEBAR_MAX: u16 = 480;
/// The terminal's font size in CSS pixels: the default and the range Settings › Terminal allows.
pub const FONT_SIZE: u8 = 11;
pub const FONT_SIZES: std::ops::RangeInclusive<u8> = 9..=24;
/// The terminal's line height in percent of the font size.
pub const LINE_HEIGHT: u16 = 100;
pub const LINE_HEIGHTS: std::ops::RangeInclusive<u16> = 100..=200;
/// Extra pixels between the terminal's characters.
pub const LETTER_SPACINGS: std::ops::RangeInclusive<i8> = -2..=4;
/// The tab bar's text size in CSS pixels, before the zoom.
pub const TAB_FONT_SIZE: u8 = 12;
pub const TAB_FONT_SIZES: std::ops::RangeInclusive<u8> = 9..=14;
/// The longest font family name Settings › Terminal takes.
pub const FONT_FAMILY_MAX: usize = 64;
/// Lines of history a terminal keeps: the default and the range. The daemon clamps to the same.
pub const SCROLLBACK: u32 = 10_000;
pub const SCROLLBACKS: std::ops::RangeInclusive<u32> = 1_000..=50_000;

/// The terminal's window opacity in percent (task 8.47); the sidebar and bars stay opaque.
pub const OPACITY: u8 = 100;
pub const OPACITIES: std::ops::RangeInclusive<u8> = 30..=100;

/// The terminal's colours: the app's own, three built in, or the ones last imported.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalTheme {
    #[default]
    Terminescu,
    Basic,
    SolarizedDark,
    SolarizedLight,
    Imported,
}

/// The terminal's cursor shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CursorStyle {
    #[default]
    Bar,
    Block,
    Underline,
}

/// Which Option key sends Esc before the key, as Meta; the other one types special characters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OptionMeta {
    #[default]
    Both,
    Left,
    Right,
    Off,
}

/// What a terminal's bell (`BEL`) does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Bell {
    #[default]
    Off,
    Visual,
    Sound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// The mode an extracted variable starts with (Settings › Variables).
    pub var_mode: Mode,
    /// The TTL a session or secret variable starts with; none keeps it until the side tab ends.
    pub var_ttl_secs: Option<u64>,
    /// `VISUAL` and `EDITOR` point at `terminescu edit --wait` (Settings › Shell, D27).
    pub editor: bool,
    /// The zoom of every window, fonts and elements, in percent (`⌘+`, `⌘-`, `⌘0`).
    pub zoom_percent: u16,
    /// The sidebar's width in CSS pixels, dragged on its right edge; a value out of range is
    /// brought into it.
    pub sidebar_width: u16,
    /// The sidebar shows only the rows' icons.
    pub sidebar_collapsed: bool,
    /// The terminal's font family; empty is the system mono font, SF Mono. A family that is not
    /// installed falls back to it.
    pub terminal_font: String,
    /// The terminal's font size in CSS pixels, before the zoom.
    pub terminal_font_size: u8,
    /// The terminal's line height in percent of the font size.
    pub terminal_line_height: u16,
    /// Extra pixels between the terminal's characters.
    pub terminal_letter_spacing: i8,
    /// The terminal's font weight, 100 to 900 in steps of 100.
    pub terminal_font_weight: u16,
    /// The tab bar's text size in CSS pixels; a tab's subtitle is one pixel smaller.
    pub tab_font_size: u8,
    /// The cursor's shape and whether it blinks; an unfocused terminal shows an outline.
    pub terminal_cursor: CursorStyle,
    pub terminal_cursor_blink: bool,
    /// Lines of history: open terminals take it at once, new shells' daemon copies too.
    pub terminal_scrollback: u32,
    pub option_as_meta: OptionMeta,
    /// The bell; visual and sound also bounce the dock icon while the app is in the background.
    pub bell: Bell,
    /// Selecting text in a terminal copies it.
    pub copy_on_select: bool,
    /// A paste of more than 5,000 characters, one that starts with `sudo` or one that pipes into
    /// a shell asks first.
    pub paste_warning: bool,
    /// Text whose colour is too close to its background is drawn with more contrast.
    pub raise_contrast: bool,
    /// The terminal's colours (task 8.47); `Imported` uses `terminal_colors`.
    pub terminal_theme: TerminalTheme,
    /// The colours last imported from Terminal.app or a file.
    pub terminal_colors: Option<crate::term_import::Colors>,
    /// Below 100 the desktop shows through the terminals' background.
    pub terminal_opacity: u8,
    /// The chords the terminal turns into shell editing keys (task 8.53).
    pub terminal_keys: TerminalKeys,
    /// Whether the person agreed to the Claude Code hook (add-public-release D5).
    pub hook: HookConsent,
    /// The first-run screen was seen to its end (D6).
    pub first_run_done: bool,
}

/// The person's answer about the Claude Code hook. `Unasked` turns into `On` when Claude
/// Code's settings already hold the hook, as on the owner's Mac before consent existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HookConsent {
    #[default]
    Unasked,
    On,
    Off,
}

impl HookConsent {
    /// The answer at launch: an existing hook entry counts as consent.
    pub fn at_launch(self, hook_present: bool) -> Self {
        match self {
            Self::Unasked if hook_present => Self::On,
            other => other,
        }
    }
}

/// Chords such as "Alt+Left" that a focused terminal turns into the bytes a shell reads; an
/// empty chord is off. They are checked against the app's chords in the UI, which knows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TerminalKeys {
    /// Back one word (ESC b).
    pub word_left: String,
    /// Forward one word (ESC f).
    pub word_right: String,
    /// To the line's start (Ctrl-A).
    pub line_start: String,
    /// To the line's end (Ctrl-E).
    pub line_end: String,
    /// Deletes to the line's start (Ctrl-U).
    pub delete_line: String,
    /// A new line without running the input (ESC CR), as Option+Enter in Claude; off by default.
    pub newline: String,
}

impl Default for TerminalKeys {
    fn default() -> Self {
        Self {
            word_left: "Alt+Left".into(),
            word_right: "Alt+Right".into(),
            line_start: "Cmd+Left".into(),
            line_end: "Cmd+Right".into(),
            delete_line: "Cmd+Backspace".into(),
            newline: String::new(),
        }
    }
}

impl TerminalKeys {
    fn all_mut(&mut self) -> [&mut String; 6] {
        [
            &mut self.word_left,
            &mut self.word_right,
            &mut self.line_start,
            &mut self.line_end,
            &mut self.delete_line,
            &mut self.newline,
        ]
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            var_mode: Mode::Saved,
            var_ttl_secs: None,
            editor: false,
            zoom_percent: 100,
            sidebar_width: SIDEBAR_WIDTH,
            sidebar_collapsed: false,
            terminal_font: String::new(),
            terminal_font_size: FONT_SIZE,
            terminal_line_height: LINE_HEIGHT,
            terminal_letter_spacing: 0,
            terminal_font_weight: 400,
            tab_font_size: TAB_FONT_SIZE,
            terminal_cursor: CursorStyle::Bar,
            terminal_cursor_blink: true,
            terminal_scrollback: SCROLLBACK,
            option_as_meta: OptionMeta::Both,
            bell: Bell::Off,
            copy_on_select: false,
            paste_warning: true,
            raise_contrast: false,
            terminal_theme: TerminalTheme::Terminescu,
            terminal_colors: None,
            terminal_opacity: OPACITY,
            terminal_keys: TerminalKeys::default(),
            hook: HookConsent::Unasked,
            first_run_done: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("a TTL runs from 1 second to 30 days")]
    BadTtl,
    #[error("the zoom runs from 50% to 200%")]
    BadZoom,
    #[error(
        "a terminal font name has letters, digits, spaces, dots, dashes and underscores, at most 64"
    )]
    BadFont,
    #[error(
        "the terminal font size runs from 9 to 24, its line height from 1.00 to 2.00, its letter spacing from -2 to 4, its weight from 100 to 900 in steps of 100, its history from 1,000 to 50,000 lines, and the tab text from 9 to 14"
    )]
    BadTerminal,
    #[error("the terminal opacity runs from 30% to 100%")]
    BadOpacity,
    #[error(
        "imported colours are #rrggbb values with 16 ANSI colours; import a theme before choosing Imported"
    )]
    BadColors,
    #[error(
        "a terminal chord has a modifier and one key (Shift alone only with a named key such as Enter), and no two terminal chords are the same"
    )]
    BadTerminalKeys,
    #[error(transparent)]
    Db(#[from] crate::db::DbError),
}

pub fn get(db: &Db) -> Result<Settings> {
    Ok(read(db, KEY)?.unwrap_or_default())
}

pub fn set(db: &Db, mut s: Settings) -> std::result::Result<Settings, SettingsError> {
    if s.var_ttl_secs.is_some_and(|t| t == 0 || t > MAX_TTL_SECS) {
        return Err(SettingsError::BadTtl);
    }
    if !(ZOOM_STEPS[0]..=ZOOM_STEPS[ZOOM_STEPS.len() - 1]).contains(&s.zoom_percent) {
        return Err(SettingsError::BadZoom);
    }
    s.terminal_font = s.terminal_font.trim().to_string();
    // The name goes into a CSS font list, so it keeps to characters that need no quoting rules.
    let name_ok = |c: char| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.');
    if s.terminal_font.chars().count() > FONT_FAMILY_MAX || !s.terminal_font.chars().all(name_ok) {
        return Err(SettingsError::BadFont);
    }
    if !FONT_SIZES.contains(&s.terminal_font_size)
        || !LINE_HEIGHTS.contains(&s.terminal_line_height)
        || !LETTER_SPACINGS.contains(&s.terminal_letter_spacing)
        || !(100..=900).contains(&s.terminal_font_weight)
        || !s.terminal_font_weight.is_multiple_of(100)
        || !TAB_FONT_SIZES.contains(&s.tab_font_size)
        || !SCROLLBACKS.contains(&s.terminal_scrollback)
    {
        return Err(SettingsError::BadTerminal);
    }
    if !OPACITIES.contains(&s.terminal_opacity) {
        return Err(SettingsError::BadOpacity);
    }
    // The colours go into CSS and the xterm theme, so each is a plain hex colour.
    if s.terminal_colors.as_ref().is_some_and(|c| !colors_ok(c))
        || (s.terminal_theme == TerminalTheme::Imported && s.terminal_colors.is_none())
    {
        return Err(SettingsError::BadColors);
    }
    let mut seen = Vec::new();
    for chord in s.terminal_keys.all_mut() {
        let c = chord.trim().to_string();
        if c.is_empty() {
            chord.clear();
            continue;
        }
        let parts: Vec<&str> = c.split('+').map(str::trim).collect();
        let n = crate::keys::normal(&c);
        let key = n.rsplit('+').next().unwrap_or("");
        // Shift with a letter is typing, so a Shift-only chord needs a named key such as Enter.
        let has_mod = parts.len() > 1 && !(n.starts_with("shift+") && key.chars().count() == 1);
        if !has_mod || key.is_empty() || c.len() > 40 || seen.contains(&n) {
            return Err(SettingsError::BadTerminalKeys);
        }
        seen.push(n);
        *chord = c;
    }
    s.sidebar_width = s.sidebar_width.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
    write(db, KEY, &s)?;
    Ok(s)
}

fn hex(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|d| d.is_ascii_hexdigit())
}

fn colors_ok(c: &crate::term_import::Colors) -> bool {
    let optional = [&c.cursor, &c.selection, &c.bold];
    c.ansi.len() == 16
        && hex(&c.foreground)
        && hex(&c.background)
        && c.ansi.iter().all(|a| hex(a))
        && optional.iter().all(|o| o.as_deref().is_none_or(hex))
}

/// One JSON value from `app_state`; a value that no longer parses reads as absent.
pub(crate) fn read<T: DeserializeOwned>(db: &Db, key: &str) -> Result<Option<T>> {
    let owner = db.local_owner()?;
    let json: Option<String> = db
        .conn()
        .query_row(
            "SELECT value FROM app_state WHERE owner_id = ?1 AND key = ?2",
            params![owner, key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
}

pub(crate) fn write<T: Serialize>(db: &Db, key: &str, value: &T) -> Result<()> {
    let owner = db.local_owner()?;
    db.conn().execute(
        "INSERT INTO app_state (owner_id, key, value) VALUES (?1, ?2, ?3)
         ON CONFLICT (owner_id, key) DO UPDATE SET value = excluded.value",
        params![
            owner,
            key,
            serde_json::to_string(value).expect("settings serialize")
        ],
    )?;
    Ok(())
}

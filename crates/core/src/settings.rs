//! App settings that are not statuses, commands or attention signals (D25): the defaults an
//! extracted variable starts with, the opt-in editor setting for the shell (D27) and the zoom
//! of every window (task 8.8), the sidebar's width and whether it shows only icons (task 8.23),
//! and the terminal's font and spacing (task 8.36). Kept as one JSON value in `app_state`.

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
pub const FONT_SIZE: u8 = 13;
pub const FONT_SIZES: std::ops::RangeInclusive<u8> = 9..=24;
/// The terminal's line height in percent of the font size.
pub const LINE_HEIGHT: u16 = 115;
pub const LINE_HEIGHTS: std::ops::RangeInclusive<u16> = 100..=200;
/// Extra pixels between the terminal's characters.
pub const LETTER_SPACINGS: std::ops::RangeInclusive<i8> = -2..=4;
/// The tab bar's text size in CSS pixels, before the zoom.
pub const TAB_FONT_SIZE: u8 = 12;
pub const TAB_FONT_SIZES: std::ops::RangeInclusive<u8> = 9..=14;
/// The longest font family name Settings › Terminal takes.
pub const FONT_FAMILY_MAX: usize = 64;

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
        "the terminal font size runs from 9 to 24, its line height from 1.00 to 2.00, its letter spacing from -2 to 4 its weight from 100 to 900 in steps of 100, and the tab text from 9 to 14"
    )]
    BadTerminal,
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
    {
        return Err(SettingsError::BadTerminal);
    }
    s.sidebar_width = s.sidebar_width.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
    write(db, KEY, &s)?;
    Ok(s)
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

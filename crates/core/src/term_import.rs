//! Import a terminal colour scheme and basic text settings from Terminal.app (its preferences
//! file or an exported `.terminal` profile) or from an iTerm2 `.itermcolors` preset.
//!
//! Terminal.app stores colours and the font as `NSKeyedArchiver` archives inside `<data>`
//! values. We do not run a general unarchiver: we scan the archive's `$objects` array for the
//! one dict that holds the colour (`NSRGB` or `NSWhite`) or the font (`NSName` + `NSSize`).
//!
//! Font rule: the PostScript name loses its style suffix after the last `-` (`Menlo-Regular`
//! becomes `Menlo`, `FiraCode-Retina` becomes `FiraCode`). `SFMono` maps to the empty string,
//! which means the system mono font. Camel-case words are not split. A result that is not
//! 1..=64 chars of ASCII letters, digits, space, `.`, `-`, `_` (empty is allowed) is dropped.

use std::io::Cursor;
use std::path::Path;

use plist::{Dictionary, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("cannot find the home folder, so Terminal.app preferences cannot be located")]
    NoHome,
    #[error("cannot read {0}: {1}")]
    Io(String, std::io::Error),
    #[error("the file is not a valid property list: {0}")]
    Parse(String),
    #[error("Terminal.app has no default profile")]
    NoDefaultProfile,
    #[error("Terminal.app's default profile \"{0}\" was not found in its preferences")]
    ProfileMissing(String),
    #[error("this file is not a Terminal.app profile or an iTerm2 colour preset")]
    NotRecognised,
    #[error("the iTerm2 colour preset is incomplete: \"{0}\" is missing or unreadable")]
    Incomplete(String),
    #[error("{0} is not a file of at most 4 MB")]
    TooBig(String),
}

/// The largest preferences file or preset read; real ones are a few KB.
const MAX_FILE: u64 = 4 * 1024 * 1024;

/// Reads a regular file of at most `MAX_FILE` bytes, so a FIFO, a device or a huge file
/// picked in the dialog cannot hang or exhaust the app.
fn read_small(path: &Path) -> Result<Vec<u8>, ImportError> {
    use std::io::Read;
    let shown = || path.display().to_string();
    // Checked before opening: opening a FIFO for reading waits for a writer.
    let meta = std::fs::metadata(path).map_err(|e| ImportError::Io(shown(), e))?;
    if !meta.is_file() || meta.len() > MAX_FILE {
        return Err(ImportError::TooBig(shown()));
    }
    let file = std::fs::File::open(path).map_err(|e| ImportError::Io(shown(), e))?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| ImportError::Io(shown(), e))?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(ImportError::TooBig(shown()));
    }
    Ok(bytes)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Colors {
    pub foreground: String,
    pub background: String,
    pub cursor: Option<String>,
    pub selection: Option<String>,
    pub bold: Option<String>,
    /// Exactly 16 entries, `#rrggbb`, normal 0-7 then bright 8-15.
    pub ansi: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CursorShape {
    Block,
    Underline,
    Bar,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    pub colors: Option<Colors>,
    /// `Some("")` means the system mono font.
    pub font_family: Option<String>,
    pub font_size: Option<f64>,
    /// Percent 0..=100 from the background colour's alpha; `None` when no background is set.
    pub background_opacity: Option<u8>,
    pub cursor: Option<CursorShape>,
    pub cursor_blink: Option<bool>,
    pub option_as_meta: Option<bool>,
}

const ANSI_KEYS: [&str; 16] = [
    "ANSIBlackColor",
    "ANSIRedColor",
    "ANSIGreenColor",
    "ANSIYellowColor",
    "ANSIBlueColor",
    "ANSIMagentaColor",
    "ANSICyanColor",
    "ANSIWhiteColor",
    "ANSIBrightBlackColor",
    "ANSIBrightRedColor",
    "ANSIBrightGreenColor",
    "ANSIBrightYellowColor",
    "ANSIBrightBlueColor",
    "ANSIBrightMagentaColor",
    "ANSIBrightCyanColor",
    "ANSIBrightWhiteColor",
];

/// Terminal.app "Basic" palette, used for any colour the profile leaves out.
const DEFAULT_ANSI: [&str; 16] = [
    "#000000", "#990000", "#00a600", "#999900", "#0000b2", "#b200b2", "#00a6b2", "#bfbfbf",
    "#666666", "#e50000", "#00d900", "#e5e500", "#0000ff", "#e500e5", "#00e5e5", "#e5e5e5",
];

/// Read the default profile from `~/Library/Preferences/com.apple.Terminal.plist`.
pub fn terminal_app_default() -> Result<Profile, ImportError> {
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .ok_or(ImportError::NoHome)?;
    let path = Path::new(&home).join("Library/Preferences/com.apple.Terminal.plist");
    let bytes = read_small(&path)?;
    terminal_app_profile_from(&bytes)
}

/// Parse the whole Terminal.app preferences file and return its default profile.
pub fn terminal_app_profile_from(plist_bytes: &[u8]) -> Result<Profile, ImportError> {
    let root = parse_plist(plist_bytes)?;
    let root = root.as_dictionary().ok_or(ImportError::NotRecognised)?;
    let name = root
        .get("Default Window Settings")
        .and_then(Value::as_string)
        .filter(|n| !n.is_empty())
        .ok_or(ImportError::NoDefaultProfile)?;
    let settings = root
        .get("Window Settings")
        .and_then(Value::as_dictionary)
        .and_then(|d| d.get(name))
        .and_then(Value::as_dictionary)
        .ok_or_else(|| ImportError::ProfileMissing(name.to_string()))?;
    Ok(profile_from_window_settings(settings, name))
}

/// Import a `.terminal` profile or an `.itermcolors` preset. The extension is a hint; the
/// content decides when it is missing or wrong.
pub fn import_file(path: &Path) -> Result<Profile, ImportError> {
    let bytes = read_small(path)?;
    let root = parse_plist(&bytes)?;
    let dict = root.as_dictionary().ok_or(ImportError::NotRecognised)?;
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Imported".to_string());
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if dict.contains_key("Ansi 0 Color") || ext == "itermcolors" {
        return profile_from_iterm(dict, &stem);
    }
    let looks_terminal = ext == "terminal"
        || dict.get("type").and_then(Value::as_string) == Some("Window Settings")
        || ANSI_KEYS.iter().any(|k| dict.contains_key(k))
        || dict.contains_key("TextColor")
        || dict.contains_key("BackgroundColor");
    if !looks_terminal {
        return Err(ImportError::NotRecognised);
    }
    let name = dict
        .get("name")
        .and_then(Value::as_string)
        .filter(|n| !n.is_empty())
        .unwrap_or(&stem)
        .to_string();
    Ok(profile_from_window_settings(dict, &name))
}

fn parse_plist(bytes: &[u8]) -> Result<Value, ImportError> {
    Value::from_reader(Cursor::new(bytes)).map_err(|e| ImportError::Parse(e.to_string()))
}

// ---- Terminal.app ----

fn profile_from_window_settings(d: &Dictionary, fallback_name: &str) -> Profile {
    let color = |key: &str| d.get(key).and_then(archived_color);
    let background = color("BackgroundColor");
    let ansi = ANSI_KEYS
        .iter()
        .zip(DEFAULT_ANSI)
        .map(|(key, default)| {
            color(key)
                .map(|c| c.hex())
                .unwrap_or_else(|| default.to_string())
        })
        .collect();
    let colors = Colors {
        foreground: color("TextColor")
            .map(|c| c.hex())
            .unwrap_or_else(|| "#000000".into()),
        background: background
            .map(|c| c.hex())
            .unwrap_or_else(|| "#ffffff".into()),
        cursor: color("CursorColor").map(|c| c.hex()),
        selection: color("SelectionColor").map(|c| c.hex()),
        bold: color("TextBoldColor").map(|c| c.hex()),
        ansi,
    };
    let font = d.get("Font").and_then(archived_font);
    let cursor = d.get("CursorType").and_then(as_i64).and_then(|n| match n {
        0 => Some(CursorShape::Block),
        1 => Some(CursorShape::Underline),
        2 => Some(CursorShape::Bar),
        _ => None,
    });
    Profile {
        name: d
            .get("name")
            .and_then(Value::as_string)
            .filter(|n| !n.is_empty())
            .unwrap_or(fallback_name)
            .to_string(),
        colors: Some(colors),
        font_family: font.as_ref().and_then(|(f, _)| family_from_postscript(f)),
        font_size: font.map(|(_, s)| s).filter(|s| s.is_finite() && *s > 0.0),
        background_opacity: background.map(|c| c.opacity_percent()),
        cursor,
        cursor_blink: d.get("CursorBlink").and_then(Value::as_boolean),
        option_as_meta: d.get("useOptionAsMetaKey").and_then(Value::as_boolean),
    }
}

fn as_i64(v: &Value) -> Option<i64> {
    v.as_signed_integer()
        .or_else(|| v.as_unsigned_integer().and_then(|u| i64::try_from(u).ok()))
}

#[derive(Clone, Copy)]
struct Rgba {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

impl Rgba {
    fn hex(self) -> String {
        let c = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02x}{:02x}{:02x}", c(self.r), c(self.g), c(self.b))
    }

    fn opacity_percent(self) -> u8 {
        (self.a.clamp(0.0, 1.0) * 100.0).round() as u8
    }
}

/// The `$objects` array of a keyed archive stored as `<data>`.
fn archive_objects(v: &Value) -> Option<Vec<Value>> {
    let bytes = v.as_data()?;
    let root = Value::from_reader(Cursor::new(bytes)).ok()?;
    root.into_dictionary()?.remove("$objects")?.into_array()
}

fn archived_color(v: &Value) -> Option<Rgba> {
    let objects = archive_objects(v)?;
    objects
        .iter()
        .filter_map(Value::as_dictionary)
        .find_map(|d| {
            if let Some(nums) = d.get("NSRGB").and_then(number_list) {
                return match nums[..] {
                    [r, g, b] => Some(Rgba { r, g, b, a: 1.0 }),
                    [r, g, b, a, ..] => Some(Rgba { r, g, b, a }),
                    _ => None,
                };
            }
            let nums = d.get("NSWhite").and_then(number_list)?;
            match nums[..] {
                [w] => Some(Rgba {
                    r: w,
                    g: w,
                    b: w,
                    a: 1.0,
                }),
                [w, a, ..] => Some(Rgba {
                    r: w,
                    g: w,
                    b: w,
                    a,
                }),
                _ => None,
            }
        })
}

/// "0.1 0.2 0.3 1" as `<data>` (possibly NUL-terminated) or as a string.
fn number_list(v: &Value) -> Option<Vec<f64>> {
    let text = match v {
        Value::Data(b) => String::from_utf8_lossy(b).into_owned(),
        Value::String(s) => s.clone(),
        _ => return None,
    };
    let nums: Vec<f64> = text
        .trim_matches(|c: char| c == '\0' || c.is_whitespace())
        .split_whitespace()
        .map(|t| t.trim_matches('\0').parse::<f64>())
        .collect::<Result<_, _>>()
        .ok()?;
    (!nums.is_empty() && nums.iter().all(|n| n.is_finite())).then_some(nums)
}

/// PostScript font name and point size from a font archive.
fn archived_font(v: &Value) -> Option<(String, f64)> {
    let objects = archive_objects(v)?;
    objects
        .iter()
        .filter_map(Value::as_dictionary)
        .find_map(|d| {
            let size = d.get("NSSize").and_then(Value::as_real)?;
            let name = match d.get("NSName")? {
                Value::String(s) => s.clone(),
                Value::Uid(u) => objects
                    .get(usize::try_from(u.get()).ok()?)?
                    .as_string()?
                    .to_string(),
                _ => return None,
            };
            Some((name, size))
        })
}

fn family_from_postscript(name: &str) -> Option<String> {
    let base = name.rsplit_once('-').map_or(name, |(b, _)| b);
    if base == "SFMono" {
        return Some(String::new());
    }
    let ok = !base.is_empty()
        && base.len() <= 64
        && base
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_'));
    ok.then(|| base.to_string())
}

// ---- iTerm2 ----

fn profile_from_iterm(d: &Dictionary, name: &str) -> Result<Profile, ImportError> {
    let need = |key: &str| -> Result<Rgba, ImportError> {
        iterm_color(d, key).ok_or_else(|| ImportError::Incomplete(key.to_string()))
    };
    let ansi = (0..16)
        .map(|i| need(&format!("Ansi {i} Color")).map(Rgba::hex))
        .collect::<Result<Vec<_>, _>>()?;
    let background = need("Background Color")?;
    let colors = Colors {
        foreground: need("Foreground Color")?.hex(),
        background: background.hex(),
        cursor: iterm_color(d, "Cursor Color").map(Rgba::hex),
        selection: iterm_color(d, "Selection Color").map(Rgba::hex),
        bold: iterm_color(d, "Bold Color").map(Rgba::hex),
        ansi,
    };
    Ok(Profile {
        name: name.to_string(),
        colors: Some(colors),
        font_family: None,
        font_size: None,
        background_opacity: Some(background.opacity_percent()),
        cursor: None,
        cursor_blink: None,
        option_as_meta: None,
    })
}

fn iterm_color(d: &Dictionary, key: &str) -> Option<Rgba> {
    let c = d.get(key)?.as_dictionary()?;
    let comp = |k: &str| c.get(k).and_then(|v| v.as_real());
    Some(Rgba {
        r: comp("Red Component")?,
        g: comp("Green Component")?,
        b: comp("Blue Component")?,
        a: comp("Alpha Component").unwrap_or(1.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use plist::Uid;

    fn bin(v: &Value) -> Vec<u8> {
        let mut out = Vec::new();
        v.to_writer_binary(&mut out).unwrap();
        out
    }

    fn dict(pairs: Vec<(&str, Value)>) -> Dictionary {
        let mut d = Dictionary::new();
        for (k, v) in pairs {
            d.insert(k.to_string(), v);
        }
        d
    }

    fn archive(objects: Vec<Value>) -> Value {
        let top = dict(vec![
            ("$archiver", Value::String("NSKeyedArchiver".into())),
            ("$objects", Value::Array(objects)),
        ]);
        Value::Data(bin(&Value::Dictionary(top)))
    }

    fn rgb(text: &str) -> Value {
        archive(vec![
            Value::String("$null".into()),
            Value::Dictionary(dict(vec![
                ("NSColorSpace", Value::Integer(1.into())),
                ("NSRGB", Value::Data(text.as_bytes().to_vec())),
            ])),
        ])
    }

    fn white(text: &str) -> Value {
        archive(vec![
            Value::String("$null".into()),
            Value::Dictionary(dict(vec![
                ("NSColorSpace", Value::Integer(3.into())),
                ("NSWhite", Value::Data(text.as_bytes().to_vec())),
            ])),
        ])
    }

    fn font(ps: &str, size: f64) -> Value {
        archive(vec![
            Value::String("$null".into()),
            Value::Dictionary(dict(vec![
                ("NSName", Value::Uid(Uid::new(2))),
                ("NSSize", Value::Real(size)),
            ])),
            Value::String(ps.into()),
        ])
    }

    fn settings() -> Dictionary {
        dict(vec![
            ("name", Value::String("Invented".into())),
            ("type", Value::String("Window Settings".into())),
            ("TextColor", rgb("1 0.5 0")),
            ("BackgroundColor", rgb("0 0 1 0.5\0")),
            ("CursorColor", white("1")),
            ("ANSIRedColor", rgb("0.2 0.4 0.6")),
            ("Font", font("Menlo-Regular", 13.0)),
            ("CursorType", Value::Integer(2.into())),
            ("CursorBlink", Value::Boolean(true)),
            ("useOptionAsMetaKey", Value::Boolean(false)),
        ])
    }

    fn prefs(default: Option<&str>) -> Vec<u8> {
        let mut windows = Dictionary::new();
        windows.insert("Invented".into(), Value::Dictionary(settings()));
        let mut root = dict(vec![("Window Settings", Value::Dictionary(windows))]);
        if let Some(n) = default {
            root.insert("Default Window Settings".into(), Value::String(n.into()));
        }
        bin(&Value::Dictionary(root))
    }

    #[test]
    fn parses_window_settings_from_preferences() {
        let p = terminal_app_profile_from(&prefs(Some("Invented"))).unwrap();
        assert_eq!(p.name, "Invented");
        let c = p.colors.unwrap();
        assert_eq!(c.foreground, "#ff8000");
        assert_eq!(c.background, "#0000ff");
        assert_eq!(c.cursor.as_deref(), Some("#ffffff"));
        assert_eq!(c.selection, None);
        assert_eq!(c.bold, None);
        assert_eq!(c.ansi.len(), 16);
        assert_eq!(c.ansi[1], "#336699");
        assert_eq!(p.font_family.as_deref(), Some("Menlo"));
        assert_eq!(p.font_size, Some(13.0));
        assert_eq!(p.background_opacity, Some(50));
        assert_eq!(p.cursor, Some(CursorShape::Bar));
        assert_eq!(p.cursor_blink, Some(true));
        assert_eq!(p.option_as_meta, Some(false));
    }

    #[test]
    fn missing_ansi_colours_use_terminal_defaults() {
        let p = terminal_app_profile_from(&prefs(Some("Invented"))).unwrap();
        let ansi = p.colors.unwrap().ansi;
        assert_eq!(ansi[0], "#000000");
        assert_eq!(ansi[2], "#00a600");
        assert_eq!(ansi[15], "#e5e5e5");
    }

    #[test]
    fn empty_profile_gets_all_defaults() {
        let p = profile_from_window_settings(&Dictionary::new(), "Bare");
        assert_eq!(p.name, "Bare");
        let c = p.colors.unwrap();
        assert_eq!(c.foreground, "#000000");
        assert_eq!(c.background, "#ffffff");
        assert_eq!(c.ansi.len(), 16);
        assert_eq!(c.ansi[8], "#666666");
        assert_eq!(p.background_opacity, None);
        assert_eq!(p.font_family, None);
        assert_eq!(p.cursor, None);
    }

    #[test]
    fn nswhite_with_and_without_alpha() {
        let c = archived_color(&white("0.5")).unwrap();
        assert_eq!(c.hex(), "#808080");
        assert_eq!(c.opacity_percent(), 100);
        let c = archived_color(&white("0 0.25\0")).unwrap();
        assert_eq!(c.hex(), "#000000");
        assert_eq!(c.opacity_percent(), 25);
    }

    #[test]
    fn nsrgb_nul_terminated_and_garbage() {
        assert_eq!(archived_color(&rgb("1 1 1\0")).unwrap().hex(), "#ffffff");
        assert!(archived_color(&rgb("not numbers")).is_none());
        assert!(archived_color(&Value::Data(vec![1, 2, 3])).is_none());
        assert!(archived_color(&Value::String("x".into())).is_none());
    }

    #[test]
    fn cursor_type_mapping() {
        for (n, want) in [
            (0, Some(CursorShape::Block)),
            (1, Some(CursorShape::Underline)),
            (2, Some(CursorShape::Bar)),
            (9, None),
        ] {
            let d = dict(vec![("CursorType", Value::Integer(n.into()))]);
            assert_eq!(profile_from_window_settings(&d, "x").cursor, want);
        }
        assert_eq!(
            serde_json::to_string(&CursorShape::Underline).unwrap(),
            "\"underline\""
        );
    }

    #[test]
    fn font_mapping() {
        assert_eq!(
            family_from_postscript("SFMono-Regular").as_deref(),
            Some("")
        );
        assert_eq!(
            family_from_postscript("Menlo-Regular").as_deref(),
            Some("Menlo")
        );
        assert_eq!(
            family_from_postscript("FiraCode-Retina").as_deref(),
            Some("FiraCode")
        );
        assert_eq!(family_from_postscript("Monaco").as_deref(), Some("Monaco"));
        assert_eq!(family_from_postscript("Bad;Font-Bold"), None);
        assert_eq!(
            family_from_postscript(&format!("{}-Bold", "a".repeat(65))),
            None
        );
        assert_eq!(family_from_postscript("-Bold"), None);
    }

    #[test]
    fn font_archive_with_inline_name() {
        let f = archive(vec![
            Value::String("$null".into()),
            Value::Dictionary(dict(vec![
                ("NSName", Value::String("Menlo-Bold".into())),
                ("NSSize", Value::Real(11.0)),
            ])),
        ]);
        assert_eq!(archived_font(&f), Some(("Menlo-Bold".into(), 11.0)));
    }

    #[test]
    fn preferences_errors() {
        assert!(matches!(
            terminal_app_profile_from(&prefs(None)),
            Err(ImportError::NoDefaultProfile)
        ));
        assert!(matches!(
            terminal_app_profile_from(&prefs(Some("Other"))),
            Err(ImportError::ProfileMissing(n)) if n == "Other"
        ));
        assert!(matches!(
            terminal_app_profile_from(b"definitely not a plist"),
            Err(ImportError::Parse(_))
        ));
    }

    const ITERM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
@COLORS@
</dict></plist>"#;

    fn iterm_entry(key: &str, r: f64, g: f64, b: f64, a: Option<f64>) -> String {
        let alpha = a
            .map(|a| format!("<key>Alpha Component</key><real>{a}</real>"))
            .unwrap_or_default();
        format!(
            "<key>{key}</key><dict><key>Blue Component</key><real>{b}</real>\
             <key>Color Space</key><string>sRGB</string>\
             <key>Green Component</key><real>{g}</real>\
             <key>Red Component</key><real>{r}</real>{alpha}</dict>"
        )
    }

    fn iterm_xml(skip: Option<&str>) -> String {
        let mut body = String::new();
        for i in 0..16 {
            let key = format!("Ansi {i} Color");
            let v = i as f64 / 15.0;
            body += &iterm_entry(&key, v, 0.0, 1.0, None);
        }
        body += &iterm_entry("Foreground Color", 1.0, 1.0, 1.0, None);
        body += &iterm_entry("Background Color", 0.0, 0.0, 0.0, Some(0.8));
        body += &iterm_entry("Cursor Color", 1.0, 0.0, 0.0, None);
        if let Some(skip) = skip {
            let start = body.find(&format!("<key>{skip}</key>")).unwrap();
            let end = body[start..].find("</dict>").unwrap() + start + "</dict>".len();
            body.replace_range(start..end, "");
        }
        ITERM.replace("@COLORS@", &body)
    }

    fn write_temp(name: &str, content: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("term_import_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn imports_itermcolors() {
        let path = write_temp("Invented Dark.itermcolors", iterm_xml(None).as_bytes());
        let p = import_file(&path).unwrap();
        assert_eq!(p.name, "Invented Dark");
        let c = p.colors.unwrap();
        assert_eq!(c.ansi.len(), 16);
        assert_eq!(c.ansi[0], "#0000ff");
        assert_eq!(c.ansi[15], "#ff00ff");
        assert_eq!(c.foreground, "#ffffff");
        assert_eq!(c.background, "#000000");
        assert_eq!(c.cursor.as_deref(), Some("#ff0000"));
        assert_eq!(c.selection, None);
        assert_eq!(p.background_opacity, Some(80));
        assert_eq!(p.font_family, None);
    }

    #[test]
    fn itermcolors_missing_required_colour_is_incomplete() {
        let path = write_temp(
            "incomplete.itermcolors",
            iterm_xml(Some("Ansi 3 Color")).as_bytes(),
        );
        assert!(matches!(
            import_file(&path),
            Err(ImportError::Incomplete(k)) if k == "Ansi 3 Color"
        ));
    }

    #[test]
    fn imports_dot_terminal_file_and_sniffs_content() {
        let bytes = bin(&Value::Dictionary(settings()));
        let by_ext = import_file(&write_temp("x.terminal", &bytes)).unwrap();
        assert_eq!(by_ext.name, "Invented");
        let sniffed = import_file(&write_temp("x.dat", &bytes)).unwrap();
        assert_eq!(sniffed, by_ext);
        let iterm = import_file(&write_temp("y.dat", iterm_xml(None).as_bytes())).unwrap();
        assert_eq!(iterm.name, "y");
    }

    #[test]
    fn import_file_errors() {
        let other = bin(&Value::Dictionary(dict(vec![(
            "Foo",
            Value::Boolean(true),
        )])));
        assert!(matches!(
            import_file(&write_temp("other.plist", &other)),
            Err(ImportError::NotRecognised)
        ));
        assert!(matches!(
            import_file(&write_temp("junk.terminal", b"junk")),
            Err(ImportError::Parse(_))
        ));
        assert!(matches!(
            import_file(Path::new("/nonexistent/none.terminal")),
            Err(ImportError::Io(..))
        ));
        assert!(matches!(
            import_file(&std::env::temp_dir()),
            Err(ImportError::TooBig(_))
        ));
        let big = write_temp("big.terminal", &vec![b' '; (MAX_FILE + 1) as usize]);
        assert!(matches!(import_file(&big), Err(ImportError::TooBig(_))));
        let msg = ImportError::NotRecognised.to_string();
        assert!(msg.contains("Terminal.app profile"));
    }

    #[test]
    fn profile_serialises_camel_case() {
        let p = terminal_app_profile_from(&prefs(Some("Invented"))).unwrap();
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["fontFamily"], "Menlo");
        assert_eq!(v["backgroundOpacity"], 50);
        assert_eq!(v["optionAsMeta"], false);
        let back: Profile = serde_json::from_value(v).unwrap();
        assert_eq!(back, p);
    }
}

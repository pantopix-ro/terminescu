//! Task 6.9: Settings › Variables and Shell keep their values in `app_state` (D25).

use terminescu_core::attention;
use terminescu_core::context::Mode;
use terminescu_core::db::Db;
use terminescu_core::settings::{self, Settings};

#[test]
fn defaults_then_a_saved_change_reads_back() {
    let db = Db::open_in_memory().unwrap();
    assert_eq!(settings::get(&db).unwrap(), Settings::default());
    let s = Settings {
        var_mode: Mode::Session,
        var_ttl_secs: Some(3600),
        editor: true,
        zoom_percent: 125,
        sidebar_width: 320,
        sidebar_collapsed: true,
        terminal_font: "JetBrains Mono".into(),
        terminal_font_size: 12,
        terminal_line_height: 100,
        terminal_letter_spacing: -1,
        terminal_font_weight: 500,
        tab_font_size: 10,
        ..Settings::default()
    };
    settings::set(&db, s.clone()).unwrap();
    assert_eq!(settings::get(&db).unwrap(), s);
    // Attention settings live under their own key and stay put.
    assert_eq!(
        attention::settings(&db).unwrap(),
        attention::Settings::default()
    );
}

#[test]
fn a_ttl_out_of_range_is_refused() {
    let db = Db::open_in_memory().unwrap();
    for ttl in [0, settings::MAX_TTL_SECS + 1] {
        let s = Settings {
            var_ttl_secs: Some(ttl),
            ..Settings::default()
        };
        assert!(settings::set(&db, s).is_err());
    }
    assert_eq!(settings::get(&db).unwrap(), Settings::default());
}

#[test]
fn json_with_unknown_fields_or_missing_ones_still_reads() {
    let s: Settings = serde_json::from_str(r#"{"varMode":"secret","future":1}"#).unwrap();
    assert_eq!(s.var_mode, Mode::Secret);
    assert!(!s.editor);
    // Settings saved before task 8.8 have no zoom: they read as 100%.
    assert_eq!(s.zoom_percent, 100);
    // And before task 8.23 no sidebar fields: the default width, not collapsed.
    assert_eq!(s.sidebar_width, settings::SIDEBAR_WIDTH);
    assert!(!s.sidebar_collapsed);
    // And before task 8.36 no terminal font: SF Mono at 11px, line height 1.00, tabs at 12px.
    assert_eq!(s.terminal_font, "");
    assert_eq!(s.terminal_font_size, settings::FONT_SIZE);
    assert_eq!(s.terminal_line_height, settings::LINE_HEIGHT);
    assert_eq!(s.tab_font_size, settings::TAB_FONT_SIZE);
}

#[test]
fn terminal_font_values_out_of_range_are_refused() {
    let db = Db::open_in_memory().unwrap();
    let bad = [
        Settings {
            terminal_font: "Menlo\", monospace; x".into(),
            ..Settings::default()
        },
        Settings {
            terminal_font: "M".repeat(65),
            ..Settings::default()
        },
        Settings {
            terminal_font_size: 8,
            ..Settings::default()
        },
        Settings {
            terminal_font_size: 25,
            ..Settings::default()
        },
        Settings {
            terminal_line_height: 99,
            ..Settings::default()
        },
        Settings {
            terminal_line_height: 201,
            ..Settings::default()
        },
        Settings {
            terminal_letter_spacing: 5,
            ..Settings::default()
        },
        Settings {
            terminal_font_weight: 450,
            ..Settings::default()
        },
        Settings {
            terminal_font_weight: 1000,
            ..Settings::default()
        },
        Settings {
            tab_font_size: 15,
            ..Settings::default()
        },
    ];
    for s in bad {
        assert!(settings::set(&db, s.clone()).is_err(), "{s:?}");
    }
    assert_eq!(settings::get(&db).unwrap(), Settings::default());
    // A name keeps no outer spaces.
    let s = Settings {
        terminal_font: "  Fira Code ".into(),
        ..Settings::default()
    };
    assert_eq!(settings::set(&db, s).unwrap().terminal_font, "Fira Code");
}

#[test]
fn a_sidebar_width_out_of_range_is_brought_into_it() {
    let db = Db::open_in_memory().unwrap();
    for (asked, kept) in [
        (0, settings::SIDEBAR_MIN),
        (900, settings::SIDEBAR_MAX),
        (300, 300),
    ] {
        let s = Settings {
            sidebar_width: asked,
            ..Settings::default()
        };
        assert_eq!(settings::set(&db, s).unwrap().sidebar_width, kept);
    }
}

#[test]
fn a_zoom_out_of_range_is_refused() {
    let db = Db::open_in_memory().unwrap();
    for zoom_percent in [0, 49, 201] {
        let s = Settings {
            zoom_percent,
            ..Settings::default()
        };
        assert!(settings::set(&db, s).is_err());
    }
    for zoom_percent in settings::ZOOM_STEPS {
        let s = Settings {
            zoom_percent,
            ..Settings::default()
        };
        assert_eq!(settings::set(&db, s).unwrap().zoom_percent, zoom_percent);
    }
}

#[test]
fn terminal_behaviour_defaults_and_round_trip() {
    use settings::{Bell, CursorStyle, OptionMeta};
    let d = Settings::default();
    assert_eq!(
        (
            d.terminal_cursor,
            d.terminal_cursor_blink,
            d.terminal_scrollback
        ),
        (CursorStyle::Bar, true, 10_000)
    );
    assert_eq!((d.option_as_meta, d.bell), (OptionMeta::Both, Bell::Off));
    assert!(d.paste_warning && !d.copy_on_select && !d.raise_contrast);
    let db = Db::open_in_memory().unwrap();
    let s = Settings {
        terminal_cursor: CursorStyle::Underline,
        terminal_cursor_blink: false,
        terminal_scrollback: 50_000,
        option_as_meta: OptionMeta::Left,
        bell: Bell::Visual,
        copy_on_select: true,
        paste_warning: false,
        raise_contrast: true,
        ..Settings::default()
    };
    settings::set(&db, s.clone()).unwrap();
    assert_eq!(settings::get(&db).unwrap(), s);
    // An older settings value without the new fields reads with their defaults.
    let old: Settings = serde_json::from_str(r#"{"zoomPercent":110}"#).unwrap();
    assert_eq!(old.terminal_scrollback, 10_000);
    assert_eq!(
        serde_json::to_value(&s).unwrap()["optionAsMeta"],
        serde_json::json!("left")
    );
}

#[test]
fn a_scrollback_out_of_range_is_refused() {
    let db = Db::open_in_memory().unwrap();
    for terminal_scrollback in [0, 999, 50_001] {
        let s = Settings {
            terminal_scrollback,
            ..Settings::default()
        };
        assert!(settings::set(&db, s).is_err());
    }
}

/// Task 8.47: theme, imported colours and opacity keep their values; bad ones are refused.
#[test]
fn theme_colours_and_opacity() {
    use terminescu_core::settings::TerminalTheme;
    use terminescu_core::term_import::Colors;
    let db = Db::open_in_memory().unwrap();
    let colors = Colors {
        foreground: "#e0e0e0".into(),
        background: "#101820".into(),
        cursor: Some("#ffcc00".into()),
        selection: None,
        bold: None,
        ansi: vec!["#202020".into(); 16],
    };
    let s = Settings {
        terminal_theme: TerminalTheme::Imported,
        terminal_colors: Some(colors.clone()),
        terminal_opacity: 85,
        ..Settings::default()
    };
    settings::set(&db, s.clone()).unwrap();
    assert_eq!(settings::get(&db).unwrap(), s);
    assert_eq!(
        serde_json::to_value(&s).unwrap()["terminalTheme"],
        serde_json::json!("imported")
    );

    let bad = [
        Settings {
            terminal_opacity: 29,
            ..Settings::default()
        },
        Settings {
            terminal_opacity: 101,
            ..Settings::default()
        },
        Settings {
            terminal_theme: TerminalTheme::Imported,
            ..Settings::default()
        },
        Settings {
            terminal_colors: Some(Colors {
                background: "red; x".into(),
                ..colors.clone()
            }),
            ..Settings::default()
        },
        Settings {
            terminal_colors: Some(Colors {
                ansi: vec!["#000000".into(); 8],
                ..colors
            }),
            ..Settings::default()
        },
    ];
    for s in bad {
        assert!(settings::set(&db, s.clone()).is_err(), "{s:?}");
    }
}

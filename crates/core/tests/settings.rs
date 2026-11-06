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
    // And before task 8.36 no terminal font: SF Mono at 13px, line height 1.15, tabs at 12px.
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

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
    };
    settings::set(&db, s).unwrap();
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
}

//! The daemon's grid gives each character the width xterm.js gives it with the Unicode 11 table
//! (task 8.41). `packages/ui/test/widths.test.ts` checks the same fixture against xterm.js.

use terminescu_ptyd::screen::Screen;

#[test]
fn the_grid_matches_the_width_fixture() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/widths.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut s = Screen::new(40, 4, 1_000);
        s.feed(case["text"].as_str().unwrap().as_bytes());
        let col = s.term().grid().cursor.point.column.0 as u64;
        assert_eq!(col, case["width"].as_u64().unwrap(), "{}", case["name"]);
    }
}

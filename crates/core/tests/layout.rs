//! The split tree of a tab (task 3.1, D3).

use terminescu_core::layout::{Dir, Layout, LayoutError, TOTAL};

fn split(l: &Layout, target: &str, dir: Dir, new: &str, sid: &str) -> Layout {
    l.split(target, dir, Layout::pane(new), sid.into()).unwrap()
}

/// A compact picture: `a`, `row(a 500, b 500)`, `col(...)`.
fn shape(l: &Layout) -> String {
    match l {
        Layout::Pane { id, .. } => id.clone(),
        Layout::Split {
            dir,
            children,
            sizes,
            ..
        } => {
            let name = match dir {
                Dir::Row => "row",
                Dir::Column => "col",
            };
            let inner: Vec<String> = children
                .iter()
                .zip(sizes)
                .map(|(c, s)| format!("{} {s}", shape(c)))
                .collect();
            format!("{name}({})", inner.join(", "))
        }
    }
}

fn sums_ok(l: &Layout) -> bool {
    match l {
        Layout::Pane { .. } => true,
        Layout::Split {
            children, sizes, ..
        } => {
            children.len() >= 2
                && sizes.len() == children.len()
                && sizes.iter().sum::<u16>() == TOTAL
                && children.iter().all(sums_ok)
        }
    }
}

#[test]
fn splitting_one_pane_makes_a_split_of_two() {
    let l = split(&Layout::pane("a"), "a", Dir::Row, "b", "s1");
    assert_eq!(shape(&l), "row(a 500, b 500)");
}

#[test]
fn splitting_in_the_parent_direction_adds_a_sibling() {
    let l = split(&Layout::pane("a"), "a", Dir::Row, "b", "s1");
    let l = split(&l, "a", Dir::Row, "c", "s2");
    assert_eq!(shape(&l), "row(a 250, c 250, b 500)");
    assert!(!l.has_split("s2"), "no new split was needed");
}

#[test]
fn splitting_across_wraps_the_pane() {
    let l = split(&Layout::pane("a"), "a", Dir::Row, "b", "s1");
    let l = split(&l, "b", Dir::Column, "c", "s2");
    assert_eq!(shape(&l), "row(a 500, col(b 500, c 500) 500)");
    assert_eq!(l.panes(), ["a", "b", "c"]);
}

#[test]
fn removing_gives_the_room_to_the_previous_pane() {
    let l = split(&Layout::pane("a"), "a", Dir::Row, "b", "s1");
    let l = split(&l, "b", Dir::Row, "c", "s2");
    assert_eq!(shape(&l), "row(a 500, b 250, c 250)");
    let l = l.remove("b").unwrap().unwrap();
    assert_eq!(shape(&l), "row(a 750, c 250)");
    // The first pane hands its room to the next one.
    let l = l.remove("a").unwrap().unwrap();
    assert_eq!(shape(&l), "c");
}

#[test]
fn removing_collapses_and_merges_same_direction_splits() {
    // row(a, col(b, row(c, d)))
    let l = split(&Layout::pane("a"), "a", Dir::Row, "b", "s1");
    let l = split(&l, "b", Dir::Column, "c", "s2");
    let l = split(&l, "c", Dir::Row, "d", "s3");
    assert_eq!(
        shape(&l),
        "row(a 500, col(b 500, row(c 500, d 500) 500) 500)"
    );
    // Without b the column holds one child, a row, which merges into the outer row.
    let l = l.remove("b").unwrap().unwrap();
    assert_eq!(shape(&l), "row(a 500, c 250, d 250)");
    assert!(sums_ok(&l));
}

#[test]
fn removing_the_last_pane_leaves_nothing() {
    assert_eq!(Layout::pane("a").remove("a"), Ok(None));
    assert_eq!(Layout::pane("a").remove("x"), Err(LayoutError::UnknownPane));
}

#[test]
fn resize_scales_and_refuses_tiny_panes() {
    let l = split(&Layout::pane("a"), "a", Dir::Row, "b", "s1");
    let l = l.resize("s1", &[30, 70]).unwrap();
    assert_eq!(shape(&l), "row(a 300, b 700)");
    assert_eq!(l.resize("s1", &[1, 999]), Err(LayoutError::BadSizes));
    assert_eq!(l.resize("s1", &[500]), Err(LayoutError::BadSizes));
    assert_eq!(l.resize("nope", &[1, 1]), Err(LayoutError::UnknownSplit));
}

#[test]
fn many_splits_and_removals_keep_the_tree_sound() {
    let mut l = Layout::pane("p0");
    let mut live = vec!["p0".to_string()];
    for i in 1..40 {
        let target = live[(i * 7) % live.len()].clone();
        let dir = if i % 3 == 0 { Dir::Column } else { Dir::Row };
        let id = format!("p{i}");
        match l.split(&target, dir, Layout::pane(&id), format!("s{i}")) {
            Ok(next) => l = next,
            Err(LayoutError::TooSmall) => continue,
            Err(e) => panic!("split {i}: {e:?}"),
        }
        live.push(id);
        assert!(sums_ok(&l), "after split {i}: {}", shape(&l));
        if i % 4 == 0 {
            let gone = live.remove((i * 5) % live.len());
            l = l.remove(&gone).unwrap().unwrap();
            assert!(sums_ok(&l), "after removing {gone}: {}", shape(&l));
        }
    }
    let mut panes: Vec<String> = l.panes().into_iter().cloned().collect();
    panes.sort();
    live.sort();
    assert_eq!(panes, live);
}

#[test]
fn json_shape_matches_the_typescript_host() {
    let l = split(&Layout::pane("a"), "a", Dir::Column, "b", "s1");
    assert_eq!(
        serde_json::to_string(&l).unwrap(),
        r#"{"type":"split","id":"s1","dir":"column","children":[{"type":"pane","id":"a"},{"type":"pane","id":"b"}],"sizes":[500,500]}"#
    );
}

#[test]
fn a_share_under_twice_the_minimum_does_not_split_again() {
    let mut l = Layout::pane("a");
    for i in 0..4 {
        l = split(&l, "a", Dir::Row, &format!("n{i}"), &format!("s{i}"));
    }
    assert_eq!(shape(&l), "row(a 63, n3 62, n2 125, n1 250, n0 500)");
    assert_eq!(
        l.split("a", Dir::Row, Layout::pane("x"), "sx".into()),
        Err(LayoutError::TooSmall)
    );
    // Across, the pane still splits: the new split gets the whole share.
    let l = split(&l, "a", Dir::Column, "y", "sy");
    assert!(l.has_pane("y"));
}

/// The same steps as the parity test in packages/host/test/layout.test.ts, which expects
/// the same picture: both sides round a merge the same way.
#[test]
fn rounding_matches_the_typescript_host() {
    let l = split(&Layout::pane("a"), "a", Dir::Row, "b", "s1");
    let l = l.resize("s1", &[333, 667]).unwrap();
    let l = split(&l, "b", Dir::Column, "c", "s2");
    let l = split(&l, "c", Dir::Row, "d", "s3");
    let l = split(&l, "d", Dir::Row, "e", "s4");
    let l = l.remove("b").unwrap().unwrap();
    assert_eq!(shape(&l), "row(a 333, c 335, d 166, e 166)");
}

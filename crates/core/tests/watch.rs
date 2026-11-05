//! Task 7.6: open files follow the disk (D28).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use terminescu_core::db::Db;
use terminescu_core::documents::Documents;

fn file(text: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("terminescu-watch-{}", terminescu_core::new_id()));
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join("proposal.md");
    std::fs::write(&p, text).unwrap();
    p
}

/// Writes so the size changes too: some file systems keep whole-second times.
fn rewrite(path: &PathBuf, text: &str) {
    std::thread::sleep(Duration::from_millis(20));
    std::fs::write(path, text).unwrap();
}

fn docs() -> Documents {
    Documents::new(Arc::new(Mutex::new(Db::open_in_memory().unwrap())))
}

#[test]
fn a_clean_buffer_reloads_when_its_file_changes() {
    let path = file("v1\n");
    let docs = docs();
    let doc = docs.open_file(&path, None).unwrap();
    assert!(docs.poll().is_empty());
    rewrite(&path, "v2 from Claude\n");
    let changed = docs.poll();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].text.as_deref(), Some("v2 from Claude\n"));
    assert_eq!(changed[0].rev, doc.rev + 1);
    assert!(!changed[0].conflict);
    assert!(docs.poll().is_empty(), "one change, one event");
}

#[test]
fn a_dirty_buffer_gets_the_conflict_event_not_a_reload() {
    let path = file("v1\n");
    let docs = docs();
    let doc = docs.open_file(&path, None).unwrap();
    docs.edit(&doc.id, "v1 with my edits\n".into()).unwrap();
    rewrite(&path, "v2 from Claude\n");
    let changed = docs.poll();
    assert_eq!(changed.len(), 1);
    assert!(changed[0].conflict);
    assert!(changed[0].text.is_none(), "the buffer stays as it is");
    assert_eq!(
        docs.get(&doc.id).unwrap().text.as_deref(),
        Some("v1 with my edits\n")
    );
    // Keep mine: the edits stay and now count against the new disk text.
    let kept = docs.keep_mine(&doc.id).unwrap();
    assert!(!kept.conflict && kept.dirty);
    docs.save_file(&doc.id, None).unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "v1 with my edits\n"
    );
    // Reload after another change takes the disk's text.
    docs.edit(&doc.id, "more edits\n".into()).unwrap();
    rewrite(&path, "v3 from Claude, longer\n");
    assert!(docs.poll()[0].conflict);
    let back = docs.reload(&doc.id).unwrap();
    assert_eq!(back.text.as_deref(), Some("v3 from Claude, longer\n"));
    assert!(!back.dirty && !back.conflict);
}

#[test]
fn a_deleted_file_marks_the_document_missing() {
    let path = file("v1\n");
    let docs = docs();
    docs.open_file(&path, None).unwrap();
    std::fs::remove_file(&path).unwrap();
    let changed = docs.poll();
    assert!(changed[0].missing);
    assert!(docs.poll().is_empty());
}

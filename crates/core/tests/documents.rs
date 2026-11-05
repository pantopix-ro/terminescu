//! Tasks 7.2 and 7.6: documents live in the core, survive a restart and follow the disk (D28).

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use terminescu_core::db::Db;
use terminescu_core::documents::{DocError, Documents, Source};

fn dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("terminescu-docs-{}", terminescu_core::new_id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn db() -> Arc<Mutex<Db>> {
    Arc::new(Mutex::new(Db::open_in_memory().unwrap()))
}

#[test]
fn an_unsaved_buffer_survives_a_core_restart_and_the_file_stays_as_it_was() {
    let d = dir();
    let path = d.join("review-notes.md");
    std::fs::write(&path, "# Review\n").unwrap();
    let db = db();
    let (file, untitled) = {
        let docs = Documents::new(db.clone());
        let file = docs.open_file(&path, None).unwrap();
        docs.edit(&file.id, "# Review\n\nPROJ-482 looks fine.\n".into())
            .unwrap();
        let untitled = docs.untitled(None).unwrap();
        docs.edit(&untitled.id, "a draft".into()).unwrap();
        (file.id, untitled.id)
    };
    // A new store over the same database is the core after a restart.
    let docs = Documents::new(db);
    docs.restore(|_| None).unwrap();
    let back = docs.get(&file).unwrap();
    assert_eq!(
        back.text.as_deref(),
        Some("# Review\n\nPROJ-482 looks fine.\n")
    );
    assert!(back.dirty);
    assert!(!back.conflict);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "# Review\n");
    let draft = docs.get(&untitled).unwrap();
    assert_eq!(draft.text.as_deref(), Some("a draft"));
    assert_eq!(draft.source, Source::Untitled);
    // Documents no pane shows go, rows and all.
    docs.retain(&HashSet::from([file.clone()])).unwrap();
    assert!(docs.get(&untitled).is_none());
    assert!(!docs.has_row(&untitled).unwrap());
    assert!(docs.has_row(&file).unwrap());
}

#[test]
fn a_session_only_document_never_gets_a_row_or_a_file() {
    let docs = Documents::new(db());
    let source = Source::Variable {
        side_tab_id: "side-1".into(),
        name: "ticket".into(),
    };
    let doc = docs
        .open_text(source, "ticket", "PROJ-482", true, None)
        .unwrap();
    docs.edit(&doc.id, "PROJ-483".into()).unwrap();
    assert!(!docs.has_row(&doc.id).unwrap());
    let to = dir().join("ticket.txt");
    assert!(matches!(
        docs.save_file(&doc.id, Some(&to)),
        Err(DocError::SessionOnly(_))
    ));
    assert!(!to.exists());
}

#[test]
fn a_file_changed_while_the_app_was_closed_flags_the_unsaved_buffer() {
    let d = dir();
    let path = d.join("proposal.md");
    std::fs::write(&path, "v1\n").unwrap();
    let db = db();
    let id = {
        let docs = Documents::new(db.clone());
        let doc = docs.open_file(&path, None).unwrap();
        docs.edit(&doc.id, "v1 mine\n".into()).unwrap();
        doc.id
    };
    std::fs::write(&path, "v2 from someone else\n").unwrap();
    let docs = Documents::new(db);
    docs.restore(|_| None).unwrap();
    let back = docs.get(&id).unwrap();
    assert!(back.conflict);
    assert_eq!(back.text.as_deref(), Some("v1 mine\n"));
}

#[test]
fn byte_exact_save_keeps_crlf_trailing_spaces_and_the_bom() {
    let d = dir();
    let path = d.join("notes.txt");
    let before = b"\xEF\xBB\xBFfirst line  \r\nsecond word\r\nthird\t\r\n".to_vec();
    std::fs::write(&path, &before).unwrap();
    let docs = Documents::new(db());
    let doc = docs.open_file(&path, None).unwrap();
    assert!(doc.bom);
    let text = doc.text.unwrap().replace("second word", "second change");
    docs.edit(&doc.id, text).unwrap();
    let saved = docs.save_file(&doc.id, None).unwrap();
    assert!(!saved.dirty);
    let after = std::fs::read(&path).unwrap();
    assert_eq!(
        after,
        b"\xEF\xBB\xBFfirst line  \r\nsecond change\r\nthird\t\r\n".to_vec()
    );
}

#[test]
fn binary_and_non_utf8_files_are_refused_and_large_ones_open_read_only() {
    let d = dir();
    let bin = d.join("logo.png");
    std::fs::write(&bin, b"\x89PNG\r\n\x1a\n\x00\x00").unwrap();
    let latin = d.join("latin1.txt");
    std::fs::write(&latin, b"caf\xe9\n").unwrap();
    let docs = Documents::new(db());
    assert!(matches!(
        docs.open_file(&bin, None),
        Err(DocError::Binary(_))
    ));
    assert!(matches!(
        docs.open_file(&latin, None),
        Err(DocError::NotUtf8(_))
    ));
    assert!(matches!(
        docs.open_file(std::path::Path::new("notes.md"), None),
        Err(DocError::Relative(_))
    ));
    let big = d.join("big.log");
    let line = "x".repeat(1023) + "\n";
    std::fs::write(&big, line.repeat(21 * 1024)).unwrap();
    let doc = docs.open_file(&big, None).unwrap();
    assert!(doc.read_only);
    assert!(matches!(
        docs.edit(&doc.id, "y".into()),
        Err(DocError::ReadOnly(_))
    ));
}

#[test]
fn a_missing_file_opens_empty_and_saving_creates_it() {
    let path = dir().join("COMMIT_EDITMSG");
    let docs = Documents::new(db());
    let doc = docs.open_file(&path, None).unwrap();
    assert!(doc.missing);
    docs.edit(&doc.id, "Add audit export\n".into()).unwrap();
    docs.save_file(&doc.id, None).unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "Add audit export\n"
    );
}

#[test]
fn an_untitled_buffer_takes_its_file_name_once_saved() {
    let docs = Documents::new(db());
    let doc = docs.untitled(None).unwrap();
    docs.edit(&doc.id, "draft\n".into()).unwrap();
    assert!(matches!(
        docs.save_file(&doc.id, None),
        Err(DocError::NeedsPath(_))
    ));
    let to = dir().join("draft.md");
    let saved = docs.save_file(&doc.id, Some(&to)).unwrap();
    assert_eq!(saved.title, "draft.md");
    assert!(!saved.dirty);
    assert_eq!(
        saved.source,
        Source::File {
            path: to.display().to_string()
        }
    );
}

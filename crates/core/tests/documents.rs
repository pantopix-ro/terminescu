//! Tasks 7.2 and 7.6: documents live in the core, survive a restart and follow the disk (D28).

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use terminescu_core::db::Db;
use terminescu_core::documents::{DocError, Documents, PaneViewTag, Source};

fn dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("terminescu-docs-{}", terminescu_core::new_id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn db() -> Arc<Mutex<Db>> {
    Arc::new(Mutex::new(Db::open_in_memory().unwrap()))
}

#[test]
fn a_pane_view_tag_survives_a_restart_and_clears() {
    let d = dir();
    let path = d.join("main.rs");
    std::fs::write(&path, "fn main() {}\n").unwrap();
    let db = db();
    let tag = PaneViewTag {
        kind: "git-diff".into(),
        source: r#"{"path":"main.rs"}"#.into(),
    };
    let id = {
        let docs = Documents::new(db.clone());
        let file = docs.open_file(&path, None).unwrap();
        let tagged = docs.set_pane_view(&file.id, Some(tag.clone())).unwrap();
        assert_eq!(tagged.pane_view.as_ref(), Some(&tag));
        file.id
    };
    let docs = Documents::new(db);
    docs.restore(|_| None).unwrap();
    assert_eq!(docs.get(&id).unwrap().pane_view, Some(tag));
    assert_eq!(docs.set_pane_view(&id, None).unwrap().pane_view, None);
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

#[test]
fn a_pasted_image_lands_next_to_the_file_and_an_untitled_one_on_its_first_save() {
    let d = dir();
    let path = d.join("proposal.md");
    std::fs::write(&path, "# Proposal\n").unwrap();
    let docs = Documents::new(db());
    let file = docs.open_file(&path, None).unwrap();
    let png = vec![0x89, b'P', b'N', b'G'];
    let a = docs.paste_image(&file.id, png.clone(), "PNG").unwrap();
    let b = docs.paste_image(&file.id, png.clone(), "png").unwrap();
    assert_ne!(a, b);
    assert!(a.starts_with("pasted-") && a.ends_with(".png"));
    assert_eq!(std::fs::read(d.join(&a)).unwrap(), png);
    assert!(matches!(
        docs.paste_image(&file.id, png.clone(), "svg"),
        Err(DocError::NotImage(_))
    ));

    let untitled = docs.untitled(None).unwrap();
    let c = docs.paste_image(&untitled.id, png.clone(), "jpeg").unwrap();
    assert!(c.ends_with(".jpg"));
    let notes = d.join("notes").join("draft.md");
    std::fs::create_dir_all(notes.parent().unwrap()).unwrap();
    docs.edit(&untitled.id, format!("![]({c})\n")).unwrap();
    docs.save_file(&untitled.id, Some(&notes)).unwrap();
    assert_eq!(
        std::fs::read(notes.parent().unwrap().join(&c)).unwrap(),
        png
    );

    let var = docs
        .open_text(Source::Untitled, "Untitled", "", true, None)
        .unwrap();
    assert!(matches!(
        docs.paste_image(&var.id, png, "png"),
        Err(DocError::NoFolder(_))
    ));
}

#[test]
fn an_edit_over_an_unseen_reload_keeps_the_text_and_flags_the_conflict() {
    let d = dir();
    let path = d.join("proposal.md");
    std::fs::write(&path, "v1\n").unwrap();
    let docs = Documents::new(db());
    let doc = docs.open_file(&path, None).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&path, "v2 from Claude\n").unwrap();
    // The core reloads the clean buffer while the editor still has typing in flight.
    let reloaded = docs.poll();
    assert_eq!(reloaded[0].rev, doc.rev + 1);
    let late = docs
        .edit_from(&doc.id, Some(doc.rev), "v1 and my line\n".into())
        .unwrap();
    assert!(late.conflict && late.dirty);
    assert!(matches!(
        docs.save_file(&doc.id, None),
        Err(DocError::Conflict(_))
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "v2 from Claude\n");
    docs.keep_mine(&doc.id).unwrap();
    docs.save_file(&doc.id, None).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "v1 and my line\n");
}

#[test]
fn a_save_refuses_when_the_disk_moved_before_the_poll_saw_it() {
    let d = dir();
    let path = d.join("notes.md");
    std::fs::write(&path, "one\n").unwrap();
    let docs = Documents::new(db());
    let doc = docs.open_file(&path, None).unwrap();
    docs.edit(&doc.id, "one, edited\n".into()).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&path, "two, from git\n").unwrap();
    assert!(matches!(
        docs.save_file(&doc.id, None),
        Err(DocError::Conflict(_))
    ));
    assert!(docs.get(&doc.id).unwrap().conflict);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "two, from git\n");
}

#[test]
fn one_file_opens_once_whatever_the_spelling() {
    let d = dir();
    std::fs::create_dir_all(d.join("src")).unwrap();
    let path = d.join("src").join("audit.ts");
    std::fs::write(&path, "export {};\n").unwrap();
    let link = d.join("audit-link.ts");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    let docs = Documents::new(db());
    let doc = docs
        .open_file(&d.join("src/../src/./audit.ts"), None)
        .unwrap();
    assert_eq!(
        doc.source,
        Source::File {
            path: path.display().to_string()
        }
    );
    assert_eq!(docs.find_file(&path), Some(doc.id.clone()));
    assert_eq!(docs.find_file(&link), Some(doc.id));
}

#[test]
fn a_document_kept_off_disk_loses_its_row_and_never_gets_one_back() {
    let docs = Documents::new(db());
    let source = Source::Variable {
        side_tab_id: "s1".into(),
        name: "ticket".into(),
    };
    let doc = docs
        .open_text(source, "ticket", "PROJ-482", false, None)
        .unwrap();
    docs.edit(&doc.id, "PROJ-483".into()).unwrap();
    assert!(docs.has_row(&doc.id).unwrap());
    assert!(docs.keep_off_disk(&doc.id).unwrap().session_only);
    assert!(!docs.has_row(&doc.id).unwrap());
    docs.edit(&doc.id, "PROJ-484".into()).unwrap();
    assert!(!docs.has_row(&doc.id).unwrap());
}

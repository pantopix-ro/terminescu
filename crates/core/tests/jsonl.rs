//! Task 4.3: the transcript tail, on invented fixtures shaped like spike 0.5's recordings.

use std::io::Write;
use std::path::PathBuf;
use terminescu_core::transcript::{EntryKind, Tail, parse_line};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/hooks");

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{name}")).unwrap()
}

fn temp_file() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("terminescu-jsonl-{}", terminescu_core::new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("session.jsonl")
}

#[test]
fn a_transcript_with_a_compaction_maps_to_entries() {
    let path = temp_file();
    std::fs::write(&path, fixture("transcript-compact.jsonl")).unwrap();
    let entries = Tail::new(&path).read().unwrap();
    let kinds: Vec<&EntryKind> = entries.iter().map(|e| &e.kind).collect();
    assert_eq!(
        kinds,
        [
            &EntryKind::User {
                text: "Plan PROJ-482: add the audit export endpoint to acme/api.".into()
            },
            &EntryKind::Assistant {
                text: "I will read the routes first.".into()
            },
            &EntryKind::Assistant {
                text: "The export streams CSV in batches of 5,000 rows.".into()
            },
            &EntryKind::TurnEnd,
            &EntryKind::Compaction,
            &EntryKind::CompactSummary {
                text: "This session is being continued from a previous conversation. Summary: PROJ-482 audit export planned.".into()
            },
            &EntryKind::User {
                text: "Now write the proposal.".into()
            },
        ]
    );
    assert_eq!(entries[0].id, "00000000-0000-4000-a000-000000000001");
    assert_eq!(entries[0].offset, 0);
    assert!(entries.iter().all(|e| e.timestamp.is_some()));
}

#[test]
fn the_denied_prompt_shows_as_a_tool_denial_then_a_turn_end() {
    let path = temp_file();
    std::fs::write(&path, fixture("transcript-denied.jsonl")).unwrap();
    let entries = Tail::new(&path).read().unwrap();
    let ends: Vec<_> = entries
        .iter()
        .filter(|e| e.kind.ends_waiting())
        .map(|e| &e.kind)
        .collect();
    assert_eq!(ends, [&EntryKind::ToolDenied, &EntryKind::TurnEnd]);
}

#[test]
fn the_cursor_reads_only_new_complete_lines() {
    let path = temp_file();
    let lines: Vec<Vec<u8>> = fixture("transcript-compact.jsonl")
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .map(<[u8]>::to_vec)
        .collect();
    let mut f = std::fs::File::create(&path).unwrap();
    let mut tail = Tail::new(&path);
    assert!(
        tail.read().unwrap().is_empty(),
        "an empty file has no entries"
    );

    f.write_all(&lines[0]).unwrap();
    f.write_all(b"\n").unwrap();
    // Half of the next line: Claude is still writing it.
    let (head, rest) = lines[1].split_at(40);
    f.write_all(head).unwrap();
    let first = tail.read().unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(tail.offset(), lines[0].len() as u64 + 1);

    f.write_all(rest).unwrap();
    f.write_all(b"\n").unwrap();
    let second = tail.read().unwrap();
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].id, "00000000-0000-4000-a000-000000000002");
    assert_eq!(second[0].offset, lines[0].len() as u64 + 1);
    assert!(tail.read().unwrap().is_empty(), "nothing new, nothing read");

    // A watcher that starts now sees only what comes next.
    let mut watch = Tail::from_end(&path);
    f.write_all(&lines[4]).unwrap();
    f.write_all(b"\n").unwrap();
    assert_eq!(
        watch
            .read()
            .unwrap()
            .iter()
            .map(|e| &e.kind)
            .collect::<Vec<_>>(),
        [&EntryKind::TurnEnd]
    );
}

#[test]
fn a_replaced_file_is_read_from_the_start() {
    let path = temp_file();
    std::fs::write(&path, fixture("transcript-compact.jsonl")).unwrap();
    let mut tail = Tail::new(&path);
    assert_eq!(tail.read().unwrap().len(), 7);
    // A new file under the same name, as a write-and-rename leaves it.
    let fresh = path.with_extension("new");
    std::fs::write(&fresh, fixture("transcript-denied.jsonl")).unwrap();
    std::fs::rename(&fresh, &path).unwrap();
    let again = tail.read().unwrap();
    assert!(again.iter().any(|e| e.kind == EntryKind::ToolDenied));
}

#[test]
fn junk_and_unknown_lines_are_skipped() {
    assert!(parse_line(b"not json", 0).is_none());
    assert!(parse_line(br#"{"type":"file-history-snapshot"}"#, 0).is_none());
    let e = parse_line(br#"{"type":"user","message":{"content":"hi"}}"#, 42).unwrap();
    assert_eq!(e.id, "@42");
}

#[test]
fn a_session_transcript_is_found_by_its_id() {
    let root = std::env::temp_dir().join(format!("terminescu-projects-{}", std::process::id()));
    let project = root.join("-Users-owner-src-acme-api");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(root.join("-Users-owner-src-other")).unwrap();
    std::fs::write(project.join("sess-0b5c.jsonl"), "").unwrap();
    assert_eq!(
        terminescu_core::transcript::find_in(&root, "sess-0b5c"),
        Some(project.join("sess-0b5c.jsonl"))
    );
    assert_eq!(
        terminescu_core::transcript::find_in(&root, "sess-none"),
        None
    );
    let _ = std::fs::remove_dir_all(&root);
}

//! Task 6.3: the prompt library over a real SQLite file (D14).

use terminescu_core::db::Db;
use terminescu_core::prompts::{self, NewPrompt, PromptError};

fn db() -> (Db, std::path::PathBuf) {
    let dir =
        std::env::temp_dir().join(format!("terminescu-prompts-{}", terminescu_core::new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    (Db::open(dir.join("t.db")).unwrap(), dir)
}

fn new(name: &str, tags: &[&str], body: &str) -> NewPrompt {
    NewPrompt {
        key: None,
        name: name.into(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        body: body.into(),
    }
}

#[test]
fn search_matches_tags_names_and_bodies() {
    let (db, dir) = db();
    let review = prompts::create(
        &db,
        &new(
            "Review plan",
            &["#Plan", "review"],
            "Rate your confidence in {{ticket}}.",
        ),
    )
    .unwrap();
    let ship = prompts::create(
        &db,
        &new("Ship it", &["release"], "Open PRs for {{branch}}."),
    )
    .unwrap();
    assert_eq!(review.key, "review-plan");
    assert_eq!(review.tags, ["plan", "review"]);
    assert_eq!(review.variables, ["ticket"]);

    let keys = |q: &str| -> Vec<String> {
        prompts::search(&db, q, false)
            .unwrap()
            .into_iter()
            .map(|p| p.key)
            .collect()
    };
    assert_eq!(keys("#plan"), ["review-plan"]);
    assert_eq!(
        keys("#rel"),
        Vec::<String>::new(),
        "a #tag matches whole tags"
    );
    assert_eq!(keys("rel"), ["ship-it"], "a word matches part of a tag");
    assert_eq!(keys("CONFIDENCE"), ["review-plan"]);
    assert_eq!(keys("prs #release"), ["ship-it"]);
    assert_eq!(keys("prs #plan"), Vec::<String>::new());
    assert_eq!(keys("").len(), 2);

    // Last used comes first; archived prompts leave the default list.
    prompts::mark_used(&db, &ship.id).unwrap();
    assert_eq!(keys(""), ["ship-it", "review-plan"]);
    prompts::set_archived(&db, &ship.id, true).unwrap();
    assert_eq!(keys(""), ["review-plan"]);
    assert_eq!(prompts::search(&db, "", true).unwrap()[0].use_count, 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn restore_creates_a_version_and_keeps_the_history() {
    let (db, dir) = db();
    let p = prompts::create(&db, &new("Review plan", &[], "v1")).unwrap();
    for body in ["v2", "v3", "v4", "v5"] {
        prompts::save(&db, &p.id, body).unwrap();
    }
    assert_eq!(
        prompts::save(&db, &p.id, "v5").unwrap().version,
        5,
        "no change, no version"
    );

    let restored = prompts::restore(&db, &p.id, 2).unwrap();
    assert_eq!((restored.version, restored.body.as_str()), (6, "v2"));
    let history: Vec<(u32, String)> = prompts::versions(&db, &p.id)
        .unwrap()
        .into_iter()
        .map(|v| (v.version, v.body))
        .collect();
    assert_eq!(history[0], (6, "v2".into()));
    assert_eq!(history[1], (5, "v5".into()));
    assert_eq!(history.len(), 6);
    assert!(matches!(
        prompts::restore(&db, &p.id, 9),
        Err(PromptError::NoVersion(_, 9))
    ));

    // Survives a reopen.
    drop(db);
    let db = Db::open(dir.join("t.db")).unwrap();
    assert_eq!(prompts::get(&db, &p.id).unwrap().body, "v2");
    let _ = std::fs::remove_dir_all(dir);
}

/// Task 7.9: a prompt edited in a text tab gets a version per `⌘S`, as the app's `doc_save`
/// does it: the prompt store first, then the document is marked saved.
#[test]
fn a_prompt_document_saves_a_version_each_time() {
    use std::sync::{Arc, Mutex};
    use terminescu_core::documents::{Documents, Source};
    let (db, dir) = db();
    let p = prompts::create(&db, &new("Review plan", &[], "Review {{ticket}}.")).unwrap();
    let db = Arc::new(Mutex::new(db));
    let docs = Documents::new(db.clone());
    let source = Source::Prompt {
        prompt_id: p.id.clone(),
    };
    let doc = docs
        .open_text(source.clone(), &p.name, &p.body, false, None)
        .unwrap();
    assert_eq!(docs.find_source(&source), Some(doc.id.clone()));
    for body in [
        "Review {{ticket}} for PROJ-482.",
        "Review {{ticket}} twice.",
    ] {
        assert!(docs.edit(&doc.id, body.into()).unwrap().dirty);
        let (_, text) = docs.text_of(&doc.id).unwrap();
        prompts::save(&db.lock().unwrap(), &p.id, &text).unwrap();
        assert!(!docs.mark_saved(&doc.id).unwrap().dirty);
    }
    let versions = prompts::versions(&db.lock().unwrap(), &p.id).unwrap();
    let bodies: Vec<&str> = versions.iter().map(|v| v.body.as_str()).collect();
    assert_eq!(
        bodies,
        [
            "Review {{ticket}} twice.",
            "Review {{ticket}} for PROJ-482.",
            "Review {{ticket}}."
        ]
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn duplicate_rename_and_keys() {
    let (db, dir) = db();
    let p = prompts::create(&db, &new("Review plan", &["plan"], "body")).unwrap();
    let copy = prompts::duplicate(&db, &p.id).unwrap();
    assert_eq!(
        (copy.name.as_str(), copy.key.as_str(), copy.version),
        ("Review plan copy", "review-plan-copy", 1)
    );
    assert_eq!(copy.tags, ["plan"]);
    let again = prompts::create(&db, &new("Review plan", &[], "other")).unwrap();
    assert_eq!(again.key, "review-plan-2");

    let explicit = NewPrompt {
        key: Some("review-plan".into()),
        ..new("x", &[], "y")
    };
    assert!(matches!(
        prompts::create(&db, &explicit),
        Err(PromptError::KeyTaken(_))
    ));
    assert!(matches!(
        prompts::create(&db, &new(" ", &[], "y")),
        Err(PromptError::NoName)
    ));
    assert!(matches!(
        prompts::create(&db, &new("x", &[], " \n")),
        Err(PromptError::Empty)
    ));

    let renamed = prompts::update(
        &db,
        &p.id,
        "Plan review",
        &["review".into(), "Review".into()],
    )
    .unwrap();
    assert_eq!((renamed.name.as_str(), renamed.version), ("Plan review", 1));
    assert_eq!(renamed.tags, ["review"]);
    assert_eq!(renamed.key, "review-plan", "the key stays");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn seeds_import_once_and_skip_taken_keys() {
    let (db, dir) = db();
    prompts::create(
        &db,
        &NewPrompt {
            key: Some("review-plan".into()),
            ..new("Mine", &[], "my own")
        },
    )
    .unwrap();
    let added = prompts::import_seeds(&db, prompts::SEEDS).unwrap();
    assert_eq!(added, prompts::SEEDS.len() - 1);
    assert_eq!(
        prompts::by_key(&db, "review-plan").unwrap().unwrap().body,
        "my own"
    );
    let seeded = prompts::by_key(&db, "api-contracts").unwrap().unwrap();
    assert_eq!(seeded.tags, ["seed"]);

    // A deleted seed stays deleted: the import runs once per database.
    prompts::set_archived(&db, &seeded.id, true).unwrap();
    db.conn()
        .execute("DELETE FROM prompts WHERE id = ?1", [&seeded.id])
        .unwrap();
    assert_eq!(prompts::import_seeds(&db, prompts::SEEDS).unwrap(), 0);
    assert!(prompts::by_key(&db, "api-contracts").unwrap().is_none());
    let _ = std::fs::remove_dir_all(dir);
}

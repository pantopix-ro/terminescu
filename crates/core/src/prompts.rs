//! The prompt library (D14): Markdown prompts with `{{variables}}`, kept as immutable versions.
//! Saving, restoring and duplicating add versions; nothing rewrites an old one.

use crate::db::{Db, DbError};
use crate::state::Id;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

/// Longest prompt body, in characters.
pub const MAX_BODY: usize = 200_000;

/// A prompt at its current version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    pub id: Id,
    pub key: String,
    pub name: String,
    pub tags: Vec<String>,
    pub archived: bool,
    pub version: u32,
    pub body: String,
    /// The `{{names}}` the body uses, in order of first use.
    pub variables: Vec<String>,
    pub last_used_at: Option<i64>,
    pub use_count: u32,
    pub created_at: i64,
    pub updated_at: i64,
}

/// One saved version of a prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub version: u32,
    pub body: String,
    pub created_at: i64,
}

/// A prompt to create, from the library, the insert dialog or a seed.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewPrompt {
    /// Derived from the name when absent.
    #[serde(default)]
    pub key: Option<String>,
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub body: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PromptError {
    #[error("a prompt needs a name")]
    NoName,
    #[error("the prompt body is empty")]
    Empty,
    #[error("the prompt body is longer than {MAX_BODY} characters")]
    TooLong,
    #[error("{0} is not a prompt key: lowercase letters, digits and -")]
    BadKey(String),
    #[error("a prompt with the key {0} exists")]
    KeyTaken(String),
    #[error("no prompt {0}")]
    NoPrompt(String),
    #[error("prompt {0} has no version {1}")]
    NoVersion(String, u32),
    #[error("seed {0}: {1}")]
    BadSeed(String, String),
    #[error(transparent)]
    Db(#[from] DbError),
}

impl From<rusqlite::Error> for PromptError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Db(e.into())
    }
}

pub type Result<T> = std::result::Result<T, PromptError>;

/// The `{{name}}` placeholders of a body, each once, in order of first use.
pub fn variables_of(body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find("{{") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("}}") else { break };
        let name = rest[..end].trim();
        if crate::context::is_variable_name(name) && !out.iter().any(|n| n == name) {
            out.push(name.to_string());
        }
        rest = &rest[end + 2..];
    }
    out
}

fn is_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !key.starts_with('-')
        && !key.ends_with('-')
}

/// `Review the plan!` becomes `review-the-plan`.
pub fn key_of(name: &str) -> String {
    let mut key = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            key.push(c);
        } else if !key.is_empty() && !key.ends_with('-') {
            key.push('-');
        }
        if key.len() >= 48 {
            break;
        }
    }
    let key = key.trim_end_matches('-').to_string();
    if key.is_empty() { "prompt".into() } else { key }
}

fn clean_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t = t.trim().trim_start_matches('#').to_lowercase();
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

fn check_body(body: &str) -> Result<()> {
    if body.trim().is_empty() {
        return Err(PromptError::Empty);
    }
    if body.chars().count() > MAX_BODY {
        return Err(PromptError::TooLong);
    }
    Ok(())
}

fn key_taken(db: &Db, owner: &str, key: &str) -> Result<bool> {
    Ok(db
        .conn()
        .query_row(
            "SELECT 1 FROM prompts WHERE owner_id = ?1 AND key = ?2",
            params![owner, key],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// `key`, or `key-2`, `key-3` when taken.
fn free_key(db: &Db, owner: &str, key: &str) -> Result<String> {
    if !key_taken(db, owner, key)? {
        return Ok(key.into());
    }
    for n in 2.. {
        let k = format!("{key}-{n}");
        if !key_taken(db, owner, &k)? {
            return Ok(k);
        }
    }
    unreachable!()
}

fn add_version(db: &Db, owner: &str, id: &str, version: u32, body: &str, now: i64) -> Result<()> {
    db.conn().execute(
        "INSERT INTO prompt_versions (id, owner_id, prompt_id, version, body, variables_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            crate::new_id(),
            owner,
            id,
            version,
            body,
            serde_json::to_string(&variables_of(body)).expect("names serialize"),
            now
        ],
    )?;
    Ok(())
}

/// Creates a prompt at version 1. An explicit key must be free; a derived one gets `-2` and on.
pub fn create(db: &Db, new: &NewPrompt) -> Result<Prompt> {
    let name = new.name.trim();
    if name.is_empty() {
        return Err(PromptError::NoName);
    }
    check_body(&new.body)?;
    let owner = db.local_owner()?;
    let key = match &new.key {
        Some(k) if !is_key(k) => return Err(PromptError::BadKey(k.clone())),
        Some(k) if key_taken(db, &owner, k)? => return Err(PromptError::KeyTaken(k.clone())),
        Some(k) => k.clone(),
        None => free_key(db, &owner, &key_of(name))?,
    };
    let id = crate::new_id();
    let now = crate::now_ms();
    let tx = db.conn().unchecked_transaction()?;
    tx.execute(
        "INSERT INTO prompts (id, owner_id, key, name, tags_json, archived, current_version,
                              created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, 1, ?6, ?6)",
        params![
            id,
            owner,
            key,
            name,
            serde_json::to_string(&clean_tags(&new.tags)).expect("tags serialize"),
            now
        ],
    )?;
    add_version(db, &owner, &id, 1, &new.body, now)?;
    tx.commit()?;
    get(db, &id)
}

const SELECT: &str =
    "SELECT p.id, p.key, p.name, p.tags_json, p.archived, p.current_version, v.body,
        p.last_used_at, p.use_count, p.created_at, p.updated_at
   FROM prompts p JOIN prompt_versions v ON v.prompt_id = p.id AND v.version = p.current_version";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Prompt> {
    let tags: String = r.get(3)?;
    let body: String = r.get(6)?;
    Ok(Prompt {
        id: r.get(0)?,
        key: r.get(1)?,
        name: r.get(2)?,
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        archived: r.get::<_, i64>(4)? != 0,
        version: r.get(5)?,
        variables: variables_of(&body),
        body,
        last_used_at: r.get(7)?,
        use_count: r.get(8)?,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
    })
}

pub fn get(db: &Db, id: &str) -> Result<Prompt> {
    let owner = db.local_owner()?;
    db.conn()
        .query_row(
            &format!("{SELECT} WHERE p.owner_id = ?1 AND p.id = ?2"),
            params![owner, id],
            row,
        )
        .optional()?
        .ok_or_else(|| PromptError::NoPrompt(id.into()))
}

pub fn by_key(db: &Db, key: &str) -> Result<Option<Prompt>> {
    let owner = db.local_owner()?;
    Ok(db
        .conn()
        .query_row(
            &format!("{SELECT} WHERE p.owner_id = ?1 AND p.key = ?2"),
            params![owner, key],
            row,
        )
        .optional()?)
}

fn all(db: &Db) -> Result<Vec<Prompt>> {
    let owner = db.local_owner()?;
    let mut st = db
        .conn()
        .prepare(&format!("{SELECT} WHERE p.owner_id = ?1"))?;
    let out = st
        .query_map(params![owner], row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(out)
}

/// The library search: every word must match. `#tag` matches a tag exactly; any other word
/// matches the name, a tag or the current body, ignoring case. Archived prompts show only when
/// asked. Most recently used first, then by name.
pub fn search(db: &Db, query: &str, archived: bool) -> Result<Vec<Prompt>> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut out: Vec<Prompt> = all(db)?
        .into_iter()
        .filter(|p| p.archived == archived)
        .filter(|p| {
            let name = p.name.to_lowercase();
            let body = p.body.to_lowercase();
            words.iter().all(|w| match w.strip_prefix('#') {
                Some(tag) if !tag.is_empty() => p.tags.iter().any(|t| t == tag),
                _ => {
                    name.contains(w.as_str())
                        || body.contains(w.as_str())
                        || p.tags.iter().any(|t| t.contains(w.as_str()))
                }
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.last_used_at
            .unwrap_or(0)
            .cmp(&a.last_used_at.unwrap_or(0))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(out)
}

/// Saves a new version when the body changed; an unchanged body keeps the current one.
pub fn save(db: &Db, id: &str, body: &str) -> Result<Prompt> {
    check_body(body)?;
    let current = get(db, id)?;
    if current.body == body {
        return Ok(current);
    }
    push_version(db, &current, body)
}

fn push_version(db: &Db, current: &Prompt, body: &str) -> Result<Prompt> {
    let owner = db.local_owner()?;
    let now = crate::now_ms();
    let next = current.version + 1;
    let tx = db.conn().unchecked_transaction()?;
    add_version(db, &owner, &current.id, next, body, now)?;
    tx.execute(
        "UPDATE prompts SET current_version = ?1, updated_at = ?2 WHERE id = ?3 AND owner_id = ?4",
        params![next, now, current.id, owner],
    )?;
    tx.commit()?;
    get(db, &current.id)
}

/// Renames and retags; neither makes a version.
pub fn update(db: &Db, id: &str, name: &str, tags: &[String]) -> Result<Prompt> {
    let name = name.trim();
    if name.is_empty() {
        return Err(PromptError::NoName);
    }
    let owner = db.local_owner()?;
    let n = db.conn().execute(
        "UPDATE prompts SET name = ?1, tags_json = ?2, updated_at = ?3 WHERE id = ?4 AND owner_id = ?5",
        params![
            name,
            serde_json::to_string(&clean_tags(tags)).expect("tags serialize"),
            crate::now_ms(),
            id,
            owner
        ],
    )?;
    if n == 0 {
        return Err(PromptError::NoPrompt(id.into()));
    }
    get(db, id)
}

/// Every version, newest first.
pub fn versions(db: &Db, id: &str) -> Result<Vec<Version>> {
    let owner = db.local_owner()?;
    let mut st = db.conn().prepare(
        "SELECT version, body, created_at FROM prompt_versions
          WHERE owner_id = ?1 AND prompt_id = ?2 ORDER BY version DESC",
    )?;
    let out = st
        .query_map(params![owner, id], |r| {
            Ok(Version {
                version: r.get(0)?,
                body: r.get(1)?,
                created_at: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if out.is_empty() {
        return Err(PromptError::NoPrompt(id.into()));
    }
    Ok(out)
}

/// Restores an older body as a new version, so the history only grows: restoring version 2
/// of a prompt at version 5 makes version 6.
pub fn restore(db: &Db, id: &str, version: u32) -> Result<Prompt> {
    let body = versions(db, id)?
        .into_iter()
        .find(|v| v.version == version)
        .ok_or_else(|| PromptError::NoVersion(id.into(), version))?
        .body;
    push_version(db, &get(db, id)?, &body)
}

/// A copy at version 1, named "<name> copy", with the same tags.
pub fn duplicate(db: &Db, id: &str) -> Result<Prompt> {
    let p = get(db, id)?;
    create(
        db,
        &NewPrompt {
            key: None,
            name: format!("{} copy", p.name),
            tags: p.tags,
            body: p.body,
        },
    )
}

pub fn set_archived(db: &Db, id: &str, archived: bool) -> Result<Prompt> {
    let owner = db.local_owner()?;
    let n = db.conn().execute(
        "UPDATE prompts SET archived = ?1, updated_at = ?2 WHERE id = ?3 AND owner_id = ?4",
        params![archived as i64, crate::now_ms(), id, owner],
    )?;
    if n == 0 {
        return Err(PromptError::NoPrompt(id.into()));
    }
    get(db, id)
}

/// Counts an insert of the prompt.
pub fn mark_used(db: &Db, id: &str) -> Result<()> {
    let owner = db.local_owner()?;
    db.conn().execute(
        "UPDATE prompts SET last_used_at = ?1, use_count = use_count + 1 WHERE id = ?2 AND owner_id = ?3",
        params![crate::now_ms(), id, owner],
    )?;
    Ok(())
}

/// The seed prompts shipped with the app (`seeds/prompts`).
pub const SEEDS: &[(&str, &str)] = &[
    (
        "api-contracts.md",
        include_str!("../../../seeds/prompts/api-contracts.md"),
    ),
    (
        "confidence.md",
        include_str!("../../../seeds/prompts/confidence.md"),
    ),
    (
        "finalize-prs.md",
        include_str!("../../../seeds/prompts/finalize-prs.md"),
    ),
    (
        "implement.md",
        include_str!("../../../seeds/prompts/implement.md"),
    ),
    (
        "plan-backend-openspec.md",
        include_str!("../../../seeds/prompts/plan-backend-openspec.md"),
    ),
    (
        "refine-plan.md",
        include_str!("../../../seeds/prompts/refine-plan.md"),
    ),
    (
        "review-and-open-prs.md",
        include_str!("../../../seeds/prompts/review-and-open-prs.md"),
    ),
    (
        "review-plan.md",
        include_str!("../../../seeds/prompts/review-plan.md"),
    ),
];

const SEEDED: &str = "prompts.seeded";

/// Parses a seed: `---` frontmatter with `key`, `name` and optional `tags: [a, b]`, then the
/// body. `variables` in the frontmatter is informational; the body is the source of truth.
pub fn parse_seed(file: &str, text: &str) -> Result<NewPrompt> {
    let bad = |why: &str| PromptError::BadSeed(file.into(), why.into());
    let text = text.replace("\r\n", "\n");
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| bad("no frontmatter"))?;
    let end = rest
        .find("\n---\n")
        .ok_or_else(|| bad("frontmatter is not closed"))?;
    let (head, body) = (&rest[..end], &rest[end + 5..]);
    let mut seed = NewPrompt {
        body: body.trim_end().to_string() + "\n",
        ..Default::default()
    };
    for line in head.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let v = v.trim();
        match k.trim() {
            "key" => seed.key = Some(v.into()),
            "name" => seed.name = v.trim_matches('"').into(),
            "tags" => {
                seed.tags = v
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split(',')
                    .map(|t| t.trim().trim_matches('"').to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            }
            _ => {}
        }
    }
    if seed.key.is_none() || seed.name.is_empty() {
        return Err(bad("needs key and name"));
    }
    Ok(seed)
}

/// Imports the seeds once per database. A key the person already has is left alone, and a
/// seed they deleted later does not come back. Returns how many prompts were added.
pub fn import_seeds(db: &Db, seeds: &[(&str, &str)]) -> Result<usize> {
    let owner = db.local_owner()?;
    let done = db
        .conn()
        .query_row(
            "SELECT 1 FROM app_state WHERE owner_id = ?1 AND key = ?2",
            params![owner, SEEDED],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if done {
        return Ok(0);
    }
    let mut added = 0;
    for (file, text) in seeds {
        let seed = parse_seed(file, text)?;
        if key_taken(db, &owner, seed.key.as_deref().unwrap_or_default())? {
            continue;
        }
        let mut seed = seed;
        if seed.tags.is_empty() {
            seed.tags = vec!["seed".into()];
        }
        create(db, &seed)?;
        added += 1;
    }
    db.conn().execute(
        "INSERT INTO app_state (owner_id, key, value) VALUES (?1, ?2, '1')",
        params![owner, SEEDED],
    )?;
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_are_listed_once_in_order() {
        assert_eq!(
            variables_of("{{ticket}} on {{ branch }}, again {{ticket}}; {{not a name}} {{pins}}"),
            ["ticket", "branch", "pins"]
        );
    }

    #[test]
    fn keys_come_from_names() {
        assert_eq!(key_of("Review the plan!"), "review-the-plan");
        assert_eq!(key_of("  ??  "), "prompt");
        assert!(is_key("review-plan-2"));
        assert!(!is_key("Review"));
        assert!(!is_key("-x"));
    }

    #[test]
    fn every_seed_file_is_embedded_and_parses() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../seeds/prompts");
        let mut files: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|f| f.ends_with(".md"))
            .collect();
        files.sort();
        let embedded: Vec<&str> = SEEDS.iter().map(|(f, _)| *f).collect();
        assert_eq!(files, embedded);
        for (file, text) in SEEDS {
            let seed = parse_seed(file, text).unwrap();
            assert!(is_key(seed.key.as_deref().unwrap()), "{file}");
            assert!(!seed.body.trim().is_empty(), "{file}");
        }
    }
}

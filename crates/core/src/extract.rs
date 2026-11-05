//! Extraction rules of v1 (D13): GitHub PR URLs, claude.ai artifact URLs and branch names in
//! Claude's replies and in terminal output. Each match lands once in the side tab's context as
//! an auto item.

use crate::context::{self, ContextError, ItemKind};
use crate::db::Db;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Found {
    pub kind: ItemKind,
    pub value: String,
}

/// Branch names too common to be worth keeping.
const PLAIN: [&str; 5] = ["main", "master", "develop", "HEAD", "origin"];

/// Every match in `text`, once each, in the order they appear.
pub fn find(text: &str) -> Vec<Found> {
    let mut out: Vec<Found> = Vec::new();
    let mut push = |f: Found| {
        if !out.contains(&f) {
            out.push(f);
        }
    };
    for (at, _) in text.match_indices("https://") {
        if let Some(link) = link_at(&text[at..]) {
            push(Found {
                kind: ItemKind::Link,
                value: link,
            });
        }
    }
    for name in branches(text) {
        push(Found {
            kind: ItemKind::Text,
            value: name,
        });
    }
    out
}

/// A PR or artifact URL at the start of `s`, cut to its canonical form.
fn link_at(s: &str) -> Option<String> {
    let end = s
        .find(|c: char| c.is_whitespace() || "\"'<>()[]{}`|\\".contains(c))
        .unwrap_or(s.len());
    let url = s[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']);
    let rest = url.strip_prefix("https://")?;
    let mut parts = rest.split(['/', '?', '#']);
    let host = parts.next()?;
    let seg: Vec<&str> = parts.take(4).collect();
    let id_ok = |id: &str| {
        !id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    match host {
        "github.com" => {
            let [owner, repo, "pull", n, ..] = seg.as_slice() else {
                return None;
            };
            let ok = |x: &str| {
                !x.is_empty()
                    && x.chars()
                        .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            };
            (ok(owner) && ok(repo) && !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
                .then(|| format!("https://github.com/{owner}/{repo}/pull/{n}"))
        }
        "claude.ai" => match seg.as_slice() {
            ["artifact", id, ..] if id_ok(id) => Some(format!("https://claude.ai/artifact/{id}")),
            ["public", "artifacts", id, ..] if id_ok(id) => {
                Some(format!("https://claude.ai/public/artifacts/{id}"))
            }
            ["code", "artifact", id, ..] if id_ok(id) => {
                Some(format!("https://claude.ai/code/artifact/{id}"))
            }
            _ => None,
        },
        _ => None,
    }
}

/// Branch names after `branch` (quoted, or unquoted with a `/` or `-`), `On branch`,
/// `checkout -b`, `switch -c` and git's `[new branch]`.
fn branches(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = text.to_ascii_lowercase();
    let mut take = |after: &str, loose: bool| {
        let after = after.trim_start_matches([' ', '\t']);
        let (quote, body) = match after.chars().next() {
            Some(q @ ('\'' | '"' | '`')) => (Some(q), &after[1..]),
            _ => (None, after),
        };
        let end = body
            .find(|c: char| !(c.is_ascii_alphanumeric() || "._/-".contains(c)))
            .unwrap_or(body.len());
        let mut name = &body[..end];
        if let Some(q) = quote {
            if !body[end..].starts_with(q) {
                return;
            }
        } else {
            name = name.trim_end_matches(['.', '/']);
            // In prose ("the feature branch pre-existing") only a name with a `/` counts.
            if !loose && !name.contains('/') {
                return;
            }
        }
        let name = name.strip_prefix("origin/").unwrap_or(name);
        if valid_branch(name) && !out.iter().any(|n: &String| n == name) {
            out.push(name.to_string());
        }
    };
    for (at, word) in lower.match_indices("branch") {
        let before = lower[..at].chars().next_back();
        if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            continue;
        }
        let after = &text[at + word.len()..];
        if let Some(rest) = after.strip_prefix(']') {
            // `* [new branch]      feature/x -> feature/x`
            take(rest, lower[..at].ends_with("[new "));
            continue;
        }
        if !after.starts_with([' ', '\t']) {
            continue;
        }
        take(after, lower[..at].ends_with("on "));
    }
    for flag in ["checkout -b ", "switch -c ", "checkout -B ", "switch -C "] {
        for (at, _) in text.match_indices(flag) {
            take(&text[at + flag.len()..], true);
        }
    }
    out
}

fn valid_branch(name: &str) -> bool {
    (2..=100).contains(&name.len())
        && !PLAIN.contains(&name)
        && !name.starts_with(['-', '/', '.'])
        && !name.ends_with(['/', '.'])
        && !name.contains("..")
        && !name.contains("//")
        && name.chars().any(|c| c.is_ascii_alphabetic())
}

/// Terminal output with escape sequences dropped, for matching.
pub fn strip_ansi(bytes: &[u8]) -> String {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            0x1b => {
                i += 1;
                match bytes.get(i) {
                    // CSI: parameters, then one final byte in 0x40..=0x7e.
                    Some(b'[') => {
                        i += 1;
                        while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                            i += 1;
                        }
                        i += 1;
                    }
                    // OSC, DCS and friends: up to BEL or ESC \.
                    Some(b']' | b'P' | b'_' | b'^') => {
                        i += 1;
                        while i < bytes.len() {
                            if bytes[i] == 0x07 {
                                i += 1;
                                break;
                            }
                            if bytes[i] == 0x1b && bytes.get(i + 1) == Some(&b'\\') {
                                i += 2;
                                break;
                            }
                            i += 1;
                        }
                    }
                    Some(_) => i += 1,
                    None => {}
                }
            }
            b'\r' => i += 1,
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Longest unfinished line a scanner keeps before it reads it anyway.
const MAX_CARRY: usize = 8 * 1024;
/// Matches a scanner remembers, so a busy pane does not ask the database again.
const MAX_SEEN: usize = 512;

/// Reads one pane's output line by line, with escape sequences and lines split across reads
/// handled. Cheap on lines with nothing to find.
#[derive(Default)]
pub struct Scanner {
    carry: Vec<u8>,
    seen: HashSet<Found>,
}

impl Scanner {
    /// Matches in the complete lines of `data`, each new to this scanner.
    pub fn feed(&mut self, data: &[u8]) -> Vec<Found> {
        self.carry.extend_from_slice(data);
        let cut = match self.carry.iter().rposition(|&b| b == b'\n') {
            Some(n) => n + 1,
            None if self.carry.len() > MAX_CARRY => self.carry.len(),
            None => return Vec::new(),
        };
        let lines: Vec<u8> = self.carry.drain(..cut).collect();
        if !worth_reading(&lines) {
            return Vec::new();
        }
        let found = find(&strip_ansi(&lines));
        if self.seen.len() > MAX_SEEN {
            self.seen.clear();
        }
        found
            .into_iter()
            .filter(|f| self.seen.insert(f.clone()))
            .collect()
    }
}

fn worth_reading(bytes: &[u8]) -> bool {
    let has = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
    has(b"https://github.com/")
        || has(b"https://claude.ai/")
        || has(b"branch")
        || has(b" -b ")
        || has(b" -c ")
}

/// Adds the matches a side tab does not hold yet as auto items; returns how many it added.
pub fn record(
    db: &Db,
    side_tab: &str,
    origin_tab: Option<&str>,
    found: &[Found],
) -> Result<usize, ContextError> {
    if found.is_empty() {
        return Ok(0);
    }
    let mut have: HashSet<String> = context::items(db, side_tab)?
        .into_iter()
        .map(|i| i.value)
        .collect();
    let mut added = 0;
    for f in found {
        if have.insert(f.value.clone()) {
            context::add_item(db, side_tab, f.kind, &f.value, "auto", origin_tab)?;
            added += 1;
        }
    }
    Ok(added)
}

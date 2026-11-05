//! Claude Code's session transcript, read as it grows (task 4.3, D34). A [`Tail`] keeps a byte
//! offset into the JSONL file and hands out [`Entry`] values for the lines added since the last
//! read. Only complete lines count: a line Claude is still writing waits for the next read.

use serde::Serialize;
use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// Longest stretch read in one call; a transcript that grew more catches up over later reads.
const MAX_READ: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// The line's `uuid`, else `@<offset>`: stable across reads and restarts.
    pub id: String,
    /// Where the line starts in the file.
    pub offset: u64,
    pub timestamp: Option<String>,
    #[serde(flatten)]
    pub kind: EntryKind,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EntryKind {
    /// What the person sent.
    User { text: String },
    /// Claude's reply text. Thinking and tool calls stay out in v1 (D25).
    Assistant { text: String },
    /// The person rejected a tool call: the prompt is gone and no hook says so (spike 0.5).
    ToolDenied,
    /// A turn ended.
    TurnEnd,
    /// The conversation was compacted; entries before it stay valid.
    Compaction,
    /// The summary Claude carries past a compaction.
    CompactSummary { text: String },
}

impl EntryKind {
    /// Ends a Waiting state that no hook will end (D34).
    pub fn ends_waiting(&self) -> bool {
        matches!(self, EntryKind::ToolDenied | EntryKind::TurnEnd)
    }
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    }
}

/// Maps one transcript line to an entry, or None for lines the app doesn't show or act on.
pub fn parse_line(line: &[u8], offset: u64) -> Option<Entry> {
    let v: Value = serde_json::from_slice(line).ok()?;
    // A subagent's lines stay out of the pane, but its denied prompt still ends Waiting.
    if v.get("isSidechain").and_then(Value::as_bool) == Some(true)
        && v.get("toolDenialKind").is_none()
    {
        return None;
    }
    let content = v.get("message").and_then(|m| m.get("content"));
    let kind = match v.get("type").and_then(Value::as_str)? {
        "system" => match v.get("subtype").and_then(Value::as_str) {
            Some("turn_duration") => EntryKind::TurnEnd,
            Some("compact_boundary") => EntryKind::Compaction,
            _ => return None,
        },
        "user" if v.get("toolDenialKind").is_some() => EntryKind::ToolDenied,
        "user" if v.get("isCompactSummary").and_then(Value::as_bool) == Some(true) => {
            EntryKind::CompactSummary {
                text: text_of(content?),
            }
        }
        "user" => {
            // Tool results and Claude's own notes ride on `user` lines too.
            let blocks = content?;
            if blocks.as_array().is_some_and(|b| {
                b.iter()
                    .any(|x| x.get("type").and_then(Value::as_str) == Some("tool_result"))
            }) || v.get("isMeta").and_then(Value::as_bool) == Some(true)
            {
                return None;
            }
            let text = text_of(blocks);
            if text.is_empty() || text.starts_with("[Request interrupted") {
                return None;
            }
            EntryKind::User { text }
        }
        "assistant" => {
            let text = text_of(content?);
            if text.is_empty() {
                return None;
            }
            EntryKind::Assistant { text }
        }
        _ => return None,
    };
    let id = v
        .get("uuid")
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| format!("@{offset}"));
    let timestamp = v.get("timestamp").and_then(Value::as_str).map(String::from);
    Some(Entry {
        id,
        offset,
        timestamp,
        kind,
    })
}

/// A cursor into one transcript file.
#[derive(Debug, Clone)]
pub struct Tail {
    path: PathBuf,
    offset: u64,
    inode: Option<u64>,
    /// Inside a line longer than [`MAX_READ`]: drop bytes up to its end.
    skipping: bool,
}

impl Tail {
    /// From the start of the file, for the transcript pane.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            offset: 0,
            inode: None,
            skipping: false,
        }
    }

    /// From the file's current end, for watching what comes next. A missing file starts at 0.
    pub fn from_end(path: impl Into<PathBuf>) -> Self {
        let mut tail = Self::new(path);
        if let Ok(meta) = std::fs::metadata(&tail.path) {
            tail.offset = meta.len();
            tail.inode = Some(meta.ino());
        }
        tail
    }

    /// From a byte offset an earlier read returned, for a reader that keeps its own cursor.
    pub fn at(path: impl Into<PathBuf>, offset: u64) -> Self {
        let mut tail = Self::new(path);
        tail.offset = offset;
        tail
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// The entries of every complete line added since the last read. A file that shrank or was
    /// replaced is read again from the start.
    pub fn read(&mut self) -> std::io::Result<Vec<Entry>> {
        let mut file = match std::fs::File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let meta = file.metadata()?;
        if meta.len() < self.offset || self.inode.is_some_and(|i| i != meta.ino()) {
            self.offset = 0;
            self.skipping = false;
        }
        self.inode = Some(meta.ino());
        file.seek(SeekFrom::Start(self.offset))?;
        let mut buf = Vec::new();
        file.take(MAX_READ as u64).read_to_end(&mut buf)?;
        let Some(last) = buf.iter().rposition(|&b| b == b'\n') else {
            if buf.len() == MAX_READ {
                // One line fills the whole read: skip it rather than stall here.
                self.offset += buf.len() as u64;
                self.skipping = true;
            }
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        let mut start = 0;
        for (i, &b) in buf[..=last].iter().enumerate() {
            if b != b'\n' {
                continue;
            }
            let line = &buf[start..i];
            let at = self.offset + start as u64;
            if self.skipping {
                self.skipping = false;
            } else if let Some(entry) = parse_line(line, at) {
                out.push(entry);
            }
            start = i + 1;
        }
        self.offset += last as u64 + 1;
        Ok(out)
    }
}

/// Claude Code's projects folder: `$CLAUDE_CONFIG_DIR/projects`, else `~/.claude/projects`.
pub fn projects_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir).join("projects");
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".claude/projects")
}

/// The transcript of `session` under `projects`: `<project>/<session>.jsonl`. The caller checks
/// the id first, so it never walks out of the folder.
pub fn find_in(projects: &Path, session: &str) -> Option<PathBuf> {
    let name = format!("{session}.jsonl");
    std::fs::read_dir(projects)
        .ok()?
        .flatten()
        .map(|d| d.path().join(&name))
        .find(|p| p.is_file())
}

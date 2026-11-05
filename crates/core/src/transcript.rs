//! Claude Code's session transcript, read as it grows (task 4.3, D34). A [`Tail`] keeps a byte
//! offset into the JSONL file and hands out [`Entry`] values for the lines added since the last
//! read. Only complete lines count: a line Claude is still writing waits for the next read.
//! Task 8.4 adds thinking, tool calls and tool results with their diffs.

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
    /// Claude's reply text.
    Assistant { text: String },
    /// Claude's visible reasoning (task 8.4). Redacted thinking stays out.
    Thinking { text: String },
    /// A tool Claude called: its name, the one input that says what it does, and for Edit,
    /// MultiEdit and Write the text it replaces (task 8.4).
    #[serde(rename_all = "camelCase")]
    ToolCall {
        tool_use_id: String,
        name: String,
        detail: String,
        edits: Vec<Edit>,
    },
    /// What the tool returned, with the patch Claude Code computed for a file change.
    #[serde(rename_all = "camelCase")]
    ToolResult {
        tool_use_id: String,
        text: String,
        is_error: bool,
        hunks: Vec<Hunk>,
    },
    /// The person rejected a tool call: the prompt is gone and no hook says so (spike 0.5).
    ToolDenied,
    /// A turn ended.
    TurnEnd,
    /// The conversation was compacted; entries before it stay valid.
    Compaction,
    /// The summary Claude carries past a compaction.
    CompactSummary { text: String },
}

/// One replacement a file tool asked for. A Write has an empty `old`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Edit {
    pub path: String,
    pub old: String,
    pub new: String,
}

/// A unified-diff hunk: `lines` start with ' ', '-' or '+'.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    pub old_start: u64,
    pub new_start: u64,
    pub lines: Vec<String>,
}

impl EntryKind {
    /// Ends a Waiting state that no hook will end (D34).
    pub fn ends_waiting(&self) -> bool {
        matches!(self, EntryKind::ToolDenied | EntryKind::TurnEnd)
    }
}

/// Longest text kept from one tool input, output or thinking block. A pane polls every
/// second, so a 2 MB Write must not travel whole.
const MAX_TEXT: usize = 16 * 1024;
/// Most diff lines kept per tool result.
const MAX_HUNK_LINES: usize = 400;

/// `s` cut to at most `max` bytes on a char boundary, with a note of what was left out.
fn clip(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… {} more bytes", &s[..end], s.len() - end)
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

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

/// The input that tells what a call does: the command, path, pattern or URL, else the input
/// as compact JSON.
fn detail_of(input: &Value) -> String {
    const KEYS: [&str; 9] = [
        "command",
        "file_path",
        "notebook_path",
        "path",
        "pattern",
        "url",
        "query",
        "description",
        "prompt",
    ];
    let found = KEYS.iter().find_map(|k| {
        input
            .get(*k)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    });
    match found {
        Some(s) => clip(s, 2000),
        None if input.as_object().is_none_or(|m| m.is_empty()) => String::new(),
        None => clip(&input.to_string(), 2000),
    }
}

fn edits_of(name: &str, input: &Value) -> Vec<Edit> {
    let path = str_of(input, "file_path").to_string();
    let one = |e: &Value| Edit {
        path: path.clone(),
        old: clip(str_of(e, "old_string"), MAX_TEXT),
        new: clip(str_of(e, "new_string"), MAX_TEXT),
    };
    match name {
        "Edit" => vec![one(input)],
        "MultiEdit" => input
            .get("edits")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(one).collect())
            .unwrap_or_default(),
        "Write" => vec![Edit {
            path: path.clone(),
            old: String::new(),
            new: clip(str_of(input, "content"), MAX_TEXT),
        }],
        _ => Vec::new(),
    }
}

fn result_text(content: Option<&Value>) -> String {
    let text = match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .map(|b| match b.get("type").and_then(Value::as_str) {
                Some("text") => str_of(b, "text").to_string(),
                Some(other) => format!("[{other}]"),
                None => String::new(),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    };
    clip(&text, MAX_TEXT)
}

/// The patch Claude Code stored for an edit, capped at [`MAX_HUNK_LINES`] lines in all.
fn hunks_of(result: Option<&Value>) -> Vec<Hunk> {
    let Some(patch) = result
        .and_then(|r| r.get("structuredPatch"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut left = MAX_HUNK_LINES;
    let mut out = Vec::new();
    for h in patch {
        if left == 0 {
            break;
        }
        let lines: Vec<String> = h
            .get("lines")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .take(left)
            .map(|l| clip(l, 2000))
            .collect();
        left -= lines.len();
        out.push(Hunk {
            old_start: h.get("oldStart").and_then(Value::as_u64).unwrap_or(0),
            new_start: h.get("newStart").and_then(Value::as_u64).unwrap_or(0),
            lines,
        });
    }
    out
}

/// Maps one transcript line to the entries it holds, in block order; empty for lines the app
/// doesn't show or act on. The reply text keeps the line's own id, so pins made before task
/// 8.4 still find it; thinking and tool blocks get `<id>#<block>`.
pub fn parse_line(line: &[u8], offset: u64) -> Vec<Entry> {
    let Ok(v) = serde_json::from_slice::<Value>(line) else {
        return Vec::new();
    };
    let id = v
        .get("uuid")
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| format!("@{offset}"));
    let timestamp = v.get("timestamp").and_then(Value::as_str).map(String::from);
    let entry = |id: String, kind: EntryKind| Entry {
        id,
        offset,
        timestamp: timestamp.clone(),
        kind,
    };
    kinds_of(&v)
        .into_iter()
        .map(|(block, kind)| match block {
            Some(i) => entry(format!("{id}#{i}"), kind),
            None => entry(id.clone(), kind),
        })
        .collect()
}

/// The tool_result blocks of a `user` line. The line's `toolUseResult` holds the patch, so it
/// counts only when the line has one result.
fn tool_results(v: &Value, blocks: Option<&Vec<Value>>) -> Vec<(Option<usize>, EntryKind)> {
    let results: Vec<_> = blocks
        .into_iter()
        .flatten()
        .enumerate()
        .filter(|(_, b)| b.get("type").and_then(Value::as_str) == Some("tool_result"))
        .collect();
    let patch = (results.len() == 1)
        .then(|| v.get("toolUseResult"))
        .flatten();
    results
        .into_iter()
        .map(|(i, b)| {
            let kind = EntryKind::ToolResult {
                tool_use_id: str_of(b, "tool_use_id").to_string(),
                text: result_text(b.get("content")),
                is_error: b.get("is_error").and_then(Value::as_bool) == Some(true),
                hunks: hunks_of(patch),
            };
            (Some(i), kind)
        })
        .collect()
}

/// The kinds on one line, each with its block index when it came from one block.
fn kinds_of(v: &Value) -> Vec<(Option<usize>, EntryKind)> {
    // A subagent's lines stay out of the pane, but its denied prompt still ends Waiting.
    if v.get("isSidechain").and_then(Value::as_bool) == Some(true)
        && v.get("toolDenialKind").is_none()
    {
        return Vec::new();
    }
    let content = v.get("message").and_then(|m| m.get("content"));
    let blocks = content.and_then(Value::as_array);
    let one = |kind| vec![(None, kind)];
    match v.get("type").and_then(Value::as_str) {
        Some("system") => match v.get("subtype").and_then(Value::as_str) {
            Some("turn_duration") => one(EntryKind::TurnEnd),
            Some("compact_boundary") => one(EntryKind::Compaction),
            _ => Vec::new(),
        },
        // The denial line also holds the call's rejected result, so the call shows as failed.
        Some("user") if v.get("toolDenialKind").is_some() => {
            let mut out = one(EntryKind::ToolDenied);
            out.extend(tool_results(v, blocks));
            out
        }
        Some("user") if v.get("isCompactSummary").and_then(Value::as_bool) == Some(true) => content
            .map_or_else(Vec::new, |c| {
                one(EntryKind::CompactSummary { text: text_of(c) })
            }),
        Some("user") if v.get("isMeta").and_then(Value::as_bool) == Some(true) => Vec::new(),
        Some("user") => {
            let Some(content) = content else {
                return Vec::new();
            };
            // Tool results ride on `user` lines.
            let results = tool_results(v, blocks);
            if !results.is_empty() {
                return results;
            }
            let text = text_of(content);
            if text.is_empty() || text.starts_with("[Request interrupted") {
                return Vec::new();
            }
            one(EntryKind::User { text })
        }
        Some("assistant") => {
            let Some(content) = content else {
                return Vec::new();
            };
            let Some(blocks) = blocks else {
                let text = text_of(content);
                return if text.is_empty() {
                    Vec::new()
                } else {
                    one(EntryKind::Assistant { text })
                };
            };
            // The reply text joins into one entry, placed where its first block was.
            let mut out = Vec::new();
            let mut text_at = None;
            for (i, b) in blocks.iter().enumerate() {
                match b.get("type").and_then(Value::as_str) {
                    Some("thinking") => {
                        let text = str_of(b, "thinking").trim();
                        if !text.is_empty() {
                            let text = clip(text, MAX_TEXT);
                            out.push((Some(i), EntryKind::Thinking { text }));
                        }
                    }
                    Some("text") if text_at.is_none() && !str_of(b, "text").is_empty() => {
                        text_at = Some(out.len());
                    }
                    Some("tool_use") => {
                        let name = str_of(b, "name").to_string();
                        let input = b.get("input").unwrap_or(&Value::Null);
                        let kind = EntryKind::ToolCall {
                            tool_use_id: str_of(b, "id").to_string(),
                            detail: detail_of(input),
                            edits: edits_of(&name, input),
                            name,
                        };
                        out.push((Some(i), kind));
                    }
                    _ => {}
                }
            }
            if let Some(at) = text_at {
                out.insert(
                    at,
                    (
                        None,
                        EntryKind::Assistant {
                            text: text_of(content),
                        },
                    ),
                );
            }
            out
        }
        _ => Vec::new(),
    }
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
            } else {
                out.extend(parse_line(line, at));
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

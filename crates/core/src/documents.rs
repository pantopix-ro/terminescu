//! Text documents (D28). Every text pane edits one document held here: a file, an Untitled
//! buffer, a context variable or a prompt. The editor sends its whole text after each pause;
//! the store keeps it and writes unsaved text to SQLite, so it survives a quit (hot exit). A
//! session-only document never gets a row and never reaches the disk.
//!
//! Files stay byte-exact: the store keeps the text as read, with its line endings and trailing
//! spaces, and writes it back the same way. A UTF-8 byte order mark is taken off for the editor
//! and put back on save. Binary files and other encodings are refused.
//!
//! `poll` watches open files by size and modification time: a clean buffer reloads, a dirty one
//! gets the changed-on-disk flag and keeps its text until the person picks Reload or Keep mine.
//! Lock order: documents, then the database.

use crate::db::Db;
use crate::state::Id;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// Larger files open read-only (D28).
pub const READ_ONLY_BYTES: u64 = 20 * 1024 * 1024;
/// Larger files do not open at all; the editor would not cope with them.
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;
/// A NUL byte in this many first bytes marks a binary file.
const SNIFF: usize = 8000;
const BOM: &[u8] = b"\xEF\xBB\xBF";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Source {
    File { path: String },
    Untitled,
    Variable { side_tab_id: Id, name: String },
    Prompt { prompt_id: Id },
}

impl Source {
    fn kind(&self) -> &'static str {
        match self {
            Source::File { .. } => "file",
            Source::Untitled => "untitled",
            Source::Variable { .. } => "variable",
            Source::Prompt { .. } => "prompt_version",
        }
    }
}

/// What a text pane shows. `text` is absent when the editor already has it: after an edit, or
/// in a changed-on-disk event that leaves the buffer as it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocView {
    pub id: Id,
    pub source: Source,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub dirty: bool,
    /// The file changed on disk under unsaved edits: Reload or Keep mine.
    pub conflict: bool,
    pub read_only: bool,
    pub bom: bool,
    /// The file is not on disk (yet); saving creates it.
    pub missing: bool,
    pub session_only: bool,
    /// `terminescu edit --wait` waits on this document (D27).
    pub request: bool,
    /// A problem to show above the text, such as a file that turned binary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Counts the text changes the store made itself (reloads), so the editor knows its copy
    /// is old.
    pub rev: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error("no document {0}")]
    Unknown(Id),
    #[error("{0} is a binary file")]
    Binary(String),
    #[error("{0} is not UTF-8 text")]
    NotUtf8(String),
    #[error("{0} is over 64 MB")]
    TooLarge(String),
    #[error("{0} is not a file")]
    NotAFile(String),
    #[error("{0} opened read-only")]
    ReadOnly(String),
    #[error("{0} is not an absolute path")]
    Relative(String),
    #[error("{0} needs a file name to save")]
    NeedsPath(String),
    #[error("{0} is session only and never goes to disk")]
    SessionOnly(String),
    #[error("{path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error(transparent)]
    Db(#[from] crate::db::DbError),
}

impl From<rusqlite::Error> for DocError {
    fn from(e: rusqlite::Error) -> Self {
        DocError::Db(e.into())
    }
}

/// A file as read from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disk {
    pub text: String,
    pub bom: bool,
    /// Size and modification time; `None` when the file is missing.
    pub sig: Option<String>,
    pub read_only: bool,
    pub missing: bool,
}

fn io_err(path: &Path, source: std::io::Error) -> DocError {
    DocError::Io {
        path: path.display().to_string(),
        source,
    }
}

fn sig_of(meta: &std::fs::Metadata) -> String {
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    format!("{}:{mtime}", meta.len())
}

/// Reads a text file. A missing file reads as empty with `missing` set, so an editor request
/// or `terminescu edit` can name a file that does not exist yet.
pub fn read_disk(path: &Path) -> Result<Disk, DocError> {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Disk {
                text: String::new(),
                bom: false,
                sig: None,
                read_only: false,
                missing: true,
            });
        }
        Err(e) => return Err(io_err(path, e)),
    };
    let shown = path.display().to_string();
    if !meta.is_file() {
        return Err(DocError::NotAFile(shown));
    }
    if meta.len() > MAX_BYTES {
        return Err(DocError::TooLarge(shown));
    }
    let bytes = std::fs::read(path).map_err(|e| io_err(path, e))?;
    let mut disk = decode(&shown, bytes)?;
    disk.sig = Some(sig_of(&meta));
    Ok(disk)
}

/// Text from file bytes: binary and non-UTF-8 content refused, a BOM taken off.
pub fn decode(shown: &str, bytes: Vec<u8>) -> Result<Disk, DocError> {
    if bytes[..bytes.len().min(SNIFF)].contains(&0) {
        return Err(DocError::Binary(shown.into()));
    }
    let read_only = bytes.len() as u64 > READ_ONLY_BYTES;
    let bom = bytes.starts_with(BOM);
    let body = if bom {
        bytes[BOM.len()..].to_vec()
    } else {
        bytes
    };
    let text = String::from_utf8(body).map_err(|_| DocError::NotUtf8(shown.into()))?;
    Ok(Disk {
        text,
        bom,
        sig: None,
        read_only,
        missing: false,
    })
}

fn file_title(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.into())
}

struct Doc {
    view: DocView,
    /// The text as the source has it now: the disk, the variable, the prompt's last version.
    saved: String,
    /// The disk state `saved` came from.
    sig: Option<String>,
    side_tab: Option<Id>,
    /// The text of the buffer, kept here because `view.text` only carries changes.
    text: String,
}

impl Doc {
    fn full(&self) -> DocView {
        DocView {
            text: Some(self.text.clone()),
            ..self.view.clone()
        }
    }
    fn meta(&self) -> DocView {
        DocView {
            text: None,
            ..self.view.clone()
        }
    }
    fn refresh_dirty(&mut self) {
        self.view.dirty = self.text != self.saved;
    }
}

pub struct Documents {
    db: Arc<Mutex<Db>>,
    docs: Mutex<HashMap<Id, Doc>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Documents {
    pub fn new(db: Arc<Mutex<Db>>) -> Self {
        Self {
            db,
            docs: Mutex::default(),
        }
    }

    /// Loads the documents saved for hot exit. `resolve` gives the current text of a variable
    /// or prompt; one that is gone turns its document into an Untitled buffer.
    pub fn restore(&self, resolve: impl Fn(&Source) -> Option<String>) -> Result<(), DocError> {
        type Row = (
            Id,
            Option<Id>,
            String,
            String,
            Option<String>,
            Option<String>,
        );
        let rows: Vec<Row> = {
            let db = lock(&self.db);
            let owner = db.local_owner()?;
            db.conn()
                .prepare(
                    "SELECT id, side_tab_id, source_ref, title, content, base_sig
                     FROM documents WHERE owner_id = ?1",
                )?
                .query_map([&owner], |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                })?
                .collect::<rusqlite::Result<_>>()?
        };
        let mut docs = lock(&self.docs);
        for (id, side_tab, source, title, content, base_sig) in rows {
            let source = serde_json::from_str::<Source>(&source).unwrap_or(Source::Untitled);
            let mut doc = Doc {
                view: DocView {
                    id: id.clone(),
                    source: source.clone(),
                    title,
                    text: None,
                    dirty: false,
                    conflict: false,
                    read_only: false,
                    bom: false,
                    missing: false,
                    session_only: false,
                    request: false,
                    note: None,
                    rev: 0,
                },
                saved: String::new(),
                sig: None,
                side_tab,
                text: String::new(),
            };
            match &source {
                Source::File { path } => match read_disk(Path::new(path)) {
                    Ok(disk) => {
                        doc.saved = disk.text;
                        doc.view.bom = disk.bom;
                        doc.view.read_only = disk.read_only;
                        doc.view.missing = disk.missing;
                        // Edits made before the quit meet a file someone changed since.
                        doc.view.conflict =
                            content.is_some() && base_sig.is_some() && disk.sig != base_sig;
                        doc.sig = disk.sig;
                    }
                    Err(e) => {
                        doc.view.note = Some(e.to_string());
                        doc.view.read_only = content.is_none();
                    }
                },
                Source::Untitled => {}
                Source::Variable { .. } | Source::Prompt { .. } => match resolve(&source) {
                    Some(text) => doc.saved = text,
                    None => {
                        doc.view.source = Source::Untitled;
                        doc.view.note =
                            Some(format!("{} is gone; this is its last text", doc.view.title));
                    }
                },
            }
            doc.text = content.unwrap_or_else(|| doc.saved.clone());
            doc.refresh_dirty();
            docs.insert(id, doc);
        }
        Ok(())
    }

    /// Drops every document no text pane shows, rows included. Run once the layout is back.
    pub fn retain(&self, keep: &HashSet<Id>) -> Result<(), DocError> {
        let gone: Vec<Id> = lock(&self.docs)
            .keys()
            .filter(|id| !keep.contains(*id))
            .cloned()
            .collect();
        self.close(&gone)?;
        let db = lock(&self.db);
        let owner = db.local_owner()?;
        let keep = serde_json::to_string(&keep.iter().collect::<Vec<_>>()).expect("ids serialize");
        db.conn().execute(
            "DELETE FROM documents WHERE owner_id = ?1 AND id NOT IN (SELECT value FROM json_each(?2))",
            params![owner, keep],
        )?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<DocView> {
        lock(&self.docs).get(id).map(Doc::full)
    }

    /// The side tab a document was opened for.
    pub fn side_tab_of(&self, id: &str) -> Option<Id> {
        lock(&self.docs).get(id).and_then(|d| d.side_tab.clone())
    }

    /// The open document of a file, if any.
    pub fn find_file(&self, path: &Path) -> Option<Id> {
        let want = path.display().to_string();
        lock(&self.docs)
            .values()
            .find(|d| matches!(&d.view.source, Source::File { path } if *path == want))
            .map(|d| d.view.id.clone())
    }

    fn insert(&self, doc: Doc) -> Result<DocView, DocError> {
        self.persist(&doc)?;
        let view = doc.full();
        lock(&self.docs).insert(doc.view.id.clone(), doc);
        Ok(view)
    }

    /// Opens a file by absolute path. A file that does not exist yet opens empty.
    pub fn open_file(&self, path: &Path, side_tab: Option<&str>) -> Result<DocView, DocError> {
        if !path.is_absolute() {
            return Err(DocError::Relative(path.display().to_string()));
        }
        let disk = read_disk(path)?;
        let shown = path.display().to_string();
        let doc = Doc {
            view: DocView {
                id: crate::new_id(),
                title: file_title(&shown),
                source: Source::File { path: shown },
                text: None,
                dirty: false,
                conflict: false,
                read_only: disk.read_only,
                bom: disk.bom,
                missing: disk.missing,
                session_only: false,
                request: false,
                note: None,
                rev: 0,
            },
            text: disk.text.clone(),
            saved: disk.text,
            sig: disk.sig,
            side_tab: side_tab.map(Into::into),
        };
        self.insert(doc)
    }

    /// A buffer with no file yet (`⌥⌘T`).
    pub fn untitled(&self, side_tab: Option<&str>) -> Result<DocView, DocError> {
        self.open_text(Source::Untitled, "Untitled", "", false, side_tab)
    }

    /// A document over a variable's or a prompt's text. Saving goes through their own stores,
    /// then [`Documents::mark_saved`].
    pub fn open_text(
        &self,
        source: Source,
        title: &str,
        text: &str,
        session_only: bool,
        side_tab: Option<&str>,
    ) -> Result<DocView, DocError> {
        let doc = Doc {
            view: DocView {
                id: crate::new_id(),
                source,
                title: title.into(),
                text: None,
                dirty: false,
                conflict: false,
                read_only: false,
                bom: false,
                missing: false,
                session_only,
                request: false,
                note: None,
                rev: 0,
            },
            saved: text.into(),
            text: text.into(),
            sig: None,
            side_tab: side_tab.map(Into::into),
        };
        self.insert(doc)
    }

    fn with<R>(
        &self,
        id: &str,
        f: impl FnOnce(&mut Doc) -> Result<R, DocError>,
    ) -> Result<R, DocError> {
        let mut docs = lock(&self.docs);
        let doc = docs
            .get_mut(id)
            .ok_or_else(|| DocError::Unknown(id.into()))?;
        let out = f(doc)?;
        self.persist(doc)?;
        Ok(out)
    }

    /// The editor's whole text after a pause in typing.
    pub fn edit(&self, id: &str, text: String) -> Result<DocView, DocError> {
        self.with(id, |doc| {
            if doc.view.read_only {
                return Err(DocError::ReadOnly(doc.view.title.clone()));
            }
            doc.text = text;
            doc.refresh_dirty();
            Ok(doc.meta())
        })
    }

    /// Writes a file or Untitled buffer to disk, byte for byte; `to` saves under a new path.
    pub fn save_file(&self, id: &str, to: Option<&Path>) -> Result<DocView, DocError> {
        self.with(id, |doc| {
            if doc.view.session_only {
                return Err(DocError::SessionOnly(doc.view.title.clone()));
            }
            let path = match (to, &doc.view.source) {
                (Some(p), _) if !p.is_absolute() => {
                    return Err(DocError::Relative(p.display().to_string()));
                }
                (Some(p), _) => p.to_path_buf(),
                (None, Source::File { path }) => PathBuf::from(path),
                (None, _) => return Err(DocError::NeedsPath(doc.view.title.clone())),
            };
            if doc.view.read_only && to.is_none() {
                return Err(DocError::ReadOnly(doc.view.title.clone()));
            }
            let mut bytes = Vec::with_capacity(doc.text.len() + BOM.len());
            if doc.view.bom {
                bytes.extend_from_slice(BOM);
            }
            bytes.extend_from_slice(doc.text.as_bytes());
            std::fs::write(&path, &bytes).map_err(|e| io_err(&path, e))?;
            let meta = std::fs::metadata(&path).map_err(|e| io_err(&path, e))?;
            let shown = path.display().to_string();
            doc.sig = Some(sig_of(&meta));
            doc.saved = doc.text.clone();
            doc.view.title = file_title(&shown);
            doc.view.source = Source::File { path: shown };
            doc.view.conflict = false;
            doc.view.missing = false;
            doc.view.read_only = false;
            doc.view.note = None;
            doc.refresh_dirty();
            Ok(doc.meta())
        })
    }

    /// The variable or prompt store took the text: the buffer is clean.
    pub fn mark_saved(&self, id: &str) -> Result<DocView, DocError> {
        self.with(id, |doc| {
            doc.saved = doc.text.clone();
            doc.refresh_dirty();
            Ok(doc.meta())
        })
    }

    /// The text the document holds now, for a save through another store.
    pub fn text_of(&self, id: &str) -> Option<(Source, String)> {
        lock(&self.docs)
            .get(id)
            .map(|d| (d.view.source.clone(), d.text.clone()))
    }

    /// Changed on disk: take the disk's text.
    pub fn reload(&self, id: &str) -> Result<DocView, DocError> {
        let path = self.path_of(id)?;
        let disk = read_disk(&path)?;
        self.with(id, |doc| {
            doc.text = disk.text.clone();
            doc.saved = disk.text;
            doc.sig = disk.sig;
            doc.view.bom = disk.bom;
            doc.view.read_only = disk.read_only;
            doc.view.missing = disk.missing;
            doc.view.conflict = false;
            doc.view.note = None;
            doc.view.rev += 1;
            doc.refresh_dirty();
            Ok(doc.full())
        })
    }

    /// Changed on disk: keep the buffer; it now counts as edits over the new disk text.
    pub fn keep_mine(&self, id: &str) -> Result<DocView, DocError> {
        let path = self.path_of(id)?;
        let disk = read_disk(&path)?;
        self.with(id, |doc| {
            doc.saved = disk.text;
            doc.sig = disk.sig;
            doc.view.missing = disk.missing;
            doc.view.conflict = false;
            doc.refresh_dirty();
            Ok(doc.meta())
        })
    }

    fn path_of(&self, id: &str) -> Result<PathBuf, DocError> {
        match lock(&self.docs).get(id).map(|d| d.view.source.clone()) {
            Some(Source::File { path }) => Ok(PathBuf::from(path)),
            Some(_) => Err(DocError::NotAFile(id.into())),
            None => Err(DocError::Unknown(id.into())),
        }
    }

    /// Marks or unmarks the document an editor request waits on.
    pub fn set_request(&self, id: &str, on: bool) -> Result<DocView, DocError> {
        self.with(id, |doc| {
            doc.view.request = on;
            Ok(doc.meta())
        })
    }

    /// Forgets documents whose panes closed, rows included.
    pub fn close(&self, ids: &[Id]) -> Result<(), DocError> {
        let mut docs = lock(&self.docs);
        for id in ids {
            docs.remove(id);
        }
        let db = lock(&self.db);
        let owner = db.local_owner()?;
        for id in ids {
            db.conn().execute(
                "DELETE FROM documents WHERE owner_id = ?1 AND id = ?2",
                params![owner, id],
            )?;
        }
        Ok(())
    }

    /// Checks open files against the disk. Returns the documents that changed: reloaded (with
    /// their text), flagged as changed on disk, or gone missing.
    pub fn poll(&self) -> Vec<DocView> {
        let files: Vec<(Id, PathBuf, Option<String>)> = lock(&self.docs)
            .values()
            .filter_map(|d| match &d.view.source {
                Source::File { path } => {
                    Some((d.view.id.clone(), PathBuf::from(path), d.sig.clone()))
                }
                _ => None,
            })
            .collect();
        let mut out = Vec::new();
        for (id, path, sig) in files {
            let now = std::fs::metadata(&path).ok().filter(|m| m.is_file());
            let now_sig = now.as_ref().map(sig_of);
            if now_sig == sig {
                continue;
            }
            // Read outside the lock; a save in the meantime changes `sig` and wins.
            let disk = now_sig.as_ref().map(|_| read_disk(&path));
            let mut docs = lock(&self.docs);
            let Some(doc) = docs.get_mut(&id) else {
                continue;
            };
            if doc.sig != sig {
                continue;
            }
            let view = match disk {
                None => {
                    doc.sig = None;
                    if doc.view.missing {
                        continue;
                    }
                    doc.view.missing = true;
                    Some(doc.meta())
                }
                Some(Err(e)) => {
                    doc.sig = now_sig;
                    doc.view.note = Some(e.to_string());
                    Some(doc.meta())
                }
                Some(Ok(d)) => {
                    let was_missing = doc.view.missing;
                    doc.sig = d.sig;
                    doc.view.missing = false;
                    doc.view.note = None;
                    if d.text == doc.saved {
                        was_missing.then(|| doc.meta())
                    } else if !doc.view.dirty {
                        doc.text = d.text.clone();
                        doc.saved = d.text;
                        doc.view.bom = d.bom;
                        doc.view.read_only = d.read_only;
                        doc.view.rev += 1;
                        Some(doc.full())
                    } else {
                        doc.view.conflict = true;
                        Some(doc.meta())
                    }
                }
            };
            if let Some(v) = view {
                if let Err(e) = self.persist(doc) {
                    eprintln!("terminescu: document {id} not saved for hot exit: {e}");
                }
                out.push(v);
            }
        }
        out
    }

    /// Writes the row a restart reads. Unsaved text goes in `content`; a clean file keeps only
    /// its path. Session-only documents never get a row.
    fn persist(&self, doc: &Doc) -> Result<(), DocError> {
        if doc.view.session_only {
            return Ok(());
        }
        let db = lock(&self.db);
        let owner = db.local_owner()?;
        let keep_text = doc.view.dirty || doc.view.source == Source::Untitled;
        let source = serde_json::to_string(&doc.view.source).expect("a source serializes");
        db.conn().execute(
            "INSERT INTO documents (id, owner_id, side_tab_id, source_kind, source_ref, title, content,
               dirty, base_sig, updated_at)
             VALUES (?1, ?2, (SELECT id FROM side_tabs WHERE id = ?3), ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT (id) DO UPDATE SET side_tab_id = excluded.side_tab_id,
               source_kind = excluded.source_kind, source_ref = excluded.source_ref,
               title = excluded.title, content = excluded.content, dirty = excluded.dirty,
               base_sig = excluded.base_sig, updated_at = excluded.updated_at",
            params![
                doc.view.id,
                owner,
                doc.side_tab,
                doc.view.source.kind(),
                source,
                doc.view.title,
                keep_text.then_some(&doc.text),
                doc.view.dirty,
                doc.sig,
                crate::now_ms(),
            ],
        )?;
        Ok(())
    }

    /// Whether a row exists, for tests of session-only documents.
    pub fn has_row(&self, id: &str) -> Result<bool, DocError> {
        let db = lock(&self.db);
        Ok(db
            .conn()
            .query_row("SELECT 1 FROM documents WHERE id = ?1", [id], |_| Ok(()))
            .optional()?
            .is_some())
    }
}

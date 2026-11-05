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
    #[error("{0} changed on disk: pick Reload or Keep mine before saving")]
    Conflict(String),
    #[error("{0} has no folder to put a pasted image in")]
    NoFolder(String),
    #[error("{0} is not a png, jpg, gif or webp image")]
    NotImage(String),
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

/// `path` with `.` and `..` worked out on the text alone, so one file has one spelling.
pub fn normalize(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(c);
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// The largest buffer whose unsaved text the database keeps for hot exit (D28).
pub const MAX_HOT_EXIT: usize = 2 * 1024 * 1024;

/// The largest image a paste saves.
pub const MAX_IMAGE: usize = 20 * 1024 * 1024;

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
    /// Images pasted into an Untitled buffer, by file name; the first save writes them next to
    /// the file (D29).
    images: Vec<(String, Vec<u8>)>,
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
                images: Vec::new(),
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
    /// The document open on this file, whatever spelling reaches it (`..`, a symlink).
    pub fn find_file(&self, path: &Path) -> Option<Id> {
        let want = normalize(path);
        let real = std::fs::canonicalize(&want).ok();
        lock(&self.docs)
            .values()
            .find(|d| match &d.view.source {
                Source::File { path } => {
                    let have = Path::new(path);
                    have == want
                        || real
                            .as_ref()
                            .is_some_and(|r| std::fs::canonicalize(have).ok().as_ref() == Some(r))
                }
                _ => false,
            })
            .map(|d| d.view.id.clone())
    }

    /// The open document over a variable or a prompt, if any.
    pub fn find_source(&self, source: &Source) -> Option<Id> {
        lock(&self.docs)
            .values()
            .find(|d| d.view.source == *source)
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
        let path = &normalize(path);
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
            images: Vec::new(),
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
            images: Vec::new(),
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
        self.edit_from(id, None, text)
    }

    /// An edit typed over revision `base`. When the core has loaded newer text since (a reload
    /// from disk the editor had not seen yet), the edit still wins in the buffer but the
    /// document shows the changed-on-disk bar, so nothing typed is lost and nothing is
    /// overwritten unasked.
    pub fn edit_from(
        &self,
        id: &str,
        base: Option<u64>,
        text: String,
    ) -> Result<DocView, DocError> {
        self.with(id, |doc| {
            if doc.view.read_only {
                return Err(DocError::ReadOnly(doc.view.title.clone()));
            }
            if base.is_some_and(|b| b != doc.view.rev) && text != doc.text {
                doc.view.conflict = true;
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
            if to.is_none() {
                // The disk moved since the buffer last matched it: the poll may not have seen
                // it yet. Saving now would overwrite someone else's change unasked.
                let now = std::fs::metadata(&path).ok().map(|m| sig_of(&m));
                if now.is_some() && now != doc.sig {
                    doc.view.conflict = true;
                }
                if doc.view.conflict {
                    return Err(DocError::Conflict(doc.view.title.clone()));
                }
            }
            let mut bytes = Vec::with_capacity(doc.text.len() + BOM.len());
            if doc.view.bom {
                bytes.extend_from_slice(BOM);
            }
            bytes.extend_from_slice(doc.text.as_bytes());
            // Images first: a failed one leaves the text unsaved and the images still held.
            if let Some(dir) = path.parent() {
                for (name, image) in &doc.images {
                    let to = dir.join(name);
                    // A file already there under that name stays; the link then points at it.
                    if !to.exists() {
                        std::fs::write(&to, image).map_err(|e| io_err(&to, e))?;
                    }
                }
            }
            std::fs::write(&path, &bytes).map_err(|e| io_err(&path, e))?;
            doc.images.clear();
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

    /// Saves a pasted image next to the document's file and returns its file name, for a link in
    /// the text (D29). An Untitled buffer keeps the image until its first save; a variable or a
    /// prompt has no folder and refuses.
    pub fn paste_image(&self, id: &str, bytes: Vec<u8>, ext: &str) -> Result<String, DocError> {
        let ext = match ext.to_ascii_lowercase().as_str() {
            "png" => "png",
            "jpg" | "jpeg" => "jpg",
            "gif" => "gif",
            "webp" => "webp",
            other => return Err(DocError::NotImage(other.into())),
        };
        if bytes.len() > MAX_IMAGE {
            return Err(DocError::TooLarge("the pasted image".into()));
        }
        self.with(id, |doc| {
            let dir = match &doc.view.source {
                Source::File { path } => Path::new(path).parent().map(Path::to_path_buf),
                Source::Untitled if !doc.view.session_only => None,
                _ => return Err(DocError::NoFolder(doc.view.title.clone())),
            };
            let stamp = crate::now_ms();
            let taken = |name: &str| {
                doc.images.iter().any(|(n, _)| n == name)
                    || dir.as_ref().is_some_and(|d| d.join(name).exists())
            };
            let mut name = format!("pasted-{stamp}.{ext}");
            let mut n = 2;
            while taken(&name) {
                name = format!("pasted-{stamp}-{n}.{ext}");
                n += 1;
            }
            match dir {
                Some(d) => {
                    let to = d.join(&name);
                    std::fs::write(&to, bytes).map_err(|e| io_err(&to, e))?;
                }
                None => doc.images.push((name.clone(), bytes)),
            }
            Ok(name)
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

    /// A document without its text.
    pub fn meta(&self, id: &str) -> Option<DocView> {
        lock(&self.docs).get(id).map(Doc::meta)
    }

    /// From now on the document stays in memory only, and its saved row goes (D35): a variable
    /// that became session only or secret under an open document.
    pub fn keep_off_disk(&self, id: &str) -> Result<DocView, DocError> {
        let view = self.with(id, |doc| {
            doc.view.session_only = true;
            Ok(doc.meta())
        })?;
        let db = lock(&self.db);
        let owner = db.local_owner()?;
        db.conn().execute(
            "DELETE FROM documents WHERE owner_id = ?1 AND id = ?2",
            params![owner, id],
        )?;
        Ok(view)
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
        // A very large buffer is not copied into SQLite at every typing pause; it still saves
        // with ⌘S, but unsaved edits to it do not survive a crash.
        let keep_text = (doc.view.dirty || doc.view.source == Source::Untitled)
            && doc.text.len() <= MAX_HOT_EXIT;
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

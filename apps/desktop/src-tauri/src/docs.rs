//! Text panes (D27, D28): the commands a window uses on documents and the disk watcher.

use crate::{AppState, lock};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use terminescu_core::context;
use terminescu_core::db::Db;
use terminescu_core::documents::{DocView, Documents, Source};
use terminescu_core::prompts;
use terminescu_core::runtime::Runtime;
use terminescu_core::state::{Command, Id};
use terminescu_core::vars;

/// A document changed outside the editor: reloaded, changed on disk, saved or gone missing.
pub const DOCUMENT_EVENT: &str = "host://document";
/// How often open files are checked against the disk.
const POLL: Duration = Duration::from_secs(1);
/// `⌘O` lists at most this many files; a bigger tree narrows by typing a path.
const MAX_FILES: usize = 20_000;

/// The text a variable or prompt document starts from and compares against.
fn source_text(db: &Db, source: &Source) -> Option<String> {
    match source {
        Source::Variable { side_tab_id, name } => vars::list(db, None, side_tab_id)
            .ok()?
            .into_iter()
            .find(|v| v.name == *name && v.mode == context::Mode::Saved)
            .and_then(|v| v.value),
        Source::Prompt { prompt_id } => prompts::get(db, prompt_id).ok().map(|p| p.body),
        Source::File { .. } | Source::Untitled => None,
    }
}

/// Loads the documents saved for hot exit and keeps the ones a text pane still shows. Closing
/// their last pane forgets them.
pub fn start(app: &AppHandle, db: Arc<std::sync::Mutex<Db>>, runtime: &Runtime) -> Arc<Documents> {
    let docs = Arc::new(Documents::new(db.clone()));
    if let Err(e) = docs.restore(|s| source_text(&lock(&db), s)) {
        eprintln!("terminescu: documents not restored: {e}");
    }
    let shown: HashSet<Id> = runtime
        .workspace()
        .doc_panes()
        .into_iter()
        .map(|(_, d, _)| d)
        .collect();
    if let Err(e) = docs.retain(&shown) {
        eprintln!("terminescu: old documents not dropped: {e}");
    }
    let closing = docs.clone();
    runtime.on_docs_closed(Box::new(move |ids| {
        if let Err(e) = closing.close(ids) {
            eprintln!("terminescu: closed documents not dropped: {e}");
        }
    }));
    let watched = Arc::downgrade(&docs);
    let handle = app.clone();
    std::thread::Builder::new()
        .name("doc-watch".into())
        .spawn(move || {
            while let Some(docs) = watched.upgrade() {
                for view in docs.poll() {
                    let _ = handle.emit(DOCUMENT_EVENT, &view);
                }
                drop(docs);
                std::thread::sleep(POLL);
            }
        })
        .expect("a thread starts");
    docs
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Shows a document in a new text pane: beside `beside`, or in a new tab of the side tab. The
/// document goes again when the pane cannot open.
fn show(
    state: &AppState,
    side_tab: &str,
    view: DocView,
    beside: Option<Id>,
) -> Result<DocView, String> {
    let cmd = Command::OpenText {
        side_tab_id: side_tab.into(),
        doc_id: view.id.clone(),
        beside,
    };
    if let Err(e) = state.runtime.command(cmd) {
        let _ = state.docs.close(std::slice::from_ref(&view.id));
        return Err(e.to_string());
    }
    Ok(view)
}

/// `⌥⌘T`: an Untitled buffer.
#[tauri::command(async)]
pub fn doc_new(
    state: State<'_, AppState>,
    side_tab: Id,
    beside: Option<Id>,
) -> Result<DocView, String> {
    let view = state.docs.untitled(Some(&side_tab)).map_err(err)?;
    show(&state, &side_tab, view, beside)
}

/// Opens a file by absolute path. A file already open comes back as it is, without a new pane;
/// the window focuses the pane that shows it.
#[tauri::command(async)]
pub fn doc_open_file(
    state: State<'_, AppState>,
    side_tab: Id,
    path: String,
    beside: Option<Id>,
) -> Result<DocView, String> {
    let path = PathBuf::from(path);
    if let Some(id) = state.docs.find_file(&path)
        && let Some(view) = state.docs.get(&id)
    {
        return Ok(view);
    }
    let view = state.docs.open_file(&path, Some(&side_tab)).map_err(err)?;
    show(&state, &side_tab, view, beside)
}

/// A document with its text, when a pane mounts or a window takes over a tab.
#[tauri::command(async)]
pub fn doc_get(state: State<'_, AppState>, id: Id) -> Result<DocView, String> {
    state
        .docs
        .get(&id)
        .ok_or_else(|| format!("no document {id}"))
}

/// The editor's whole text after a pause in typing.
#[tauri::command(async)]
pub fn doc_edit(state: State<'_, AppState>, id: Id, text: String) -> Result<DocView, String> {
    state.docs.edit(&id, text).map_err(err)
}

/// `⌘S`: a file to disk, a variable or prompt through its own store. `path` saves a file or an
/// Untitled buffer under a new name (Save As).
#[tauri::command(async)]
pub fn doc_save(
    app: AppHandle,
    state: State<'_, AppState>,
    id: Id,
    path: Option<String>,
) -> Result<DocView, String> {
    let (source, text) = state
        .docs
        .text_of(&id)
        .ok_or_else(|| format!("no document {id}"))?;
    let view = match (&source, path) {
        (_, Some(p)) => state
            .docs
            .save_file(&id, Some(Path::new(&p)))
            .map_err(err)?,
        (Source::File { .. } | Source::Untitled, None) => {
            state.docs.save_file(&id, None).map_err(err)?
        }
        (Source::Variable { side_tab_id, name }, None) => {
            let daemon = state.runtime.daemon();
            let change = vars::VariableChange {
                value: Some(text),
                ..Default::default()
            };
            vars::change(&*state.db, daemon.as_deref(), side_tab_id, name, &change).map_err(err)?;
            let _ = app.emit(crate::CONTEXT_EVENT, side_tab_id);
            state.docs.mark_saved(&id).map_err(err)?
        }
        (Source::Prompt { prompt_id }, None) => {
            let r = prompts::save(&lock(&state.db), prompt_id, &text);
            crate::prompt_changed(&app, r)?;
            state.docs.mark_saved(&id).map_err(err)?
        }
    };
    Ok(view)
}

/// Changed on disk: take the disk's text.
#[tauri::command(async)]
pub fn doc_reload(state: State<'_, AppState>, id: Id) -> Result<DocView, String> {
    state.docs.reload(&id).map_err(err)
}

/// Changed on disk: keep the buffer as edits over the new disk text.
#[tauri::command(async)]
pub fn doc_keep_mine(state: State<'_, AppState>, id: Id) -> Result<DocView, String> {
    state.docs.keep_mine(&id).map_err(err)
}

/// Files under a pane's cwd for `⌘O`: what git tracks or would track, else a walk that skips
/// hidden folders. Paths are relative to `root`, which comes back with them.
#[derive(serde::Serialize)]
pub struct FileList {
    root: String,
    files: Vec<String>,
    /// More files exist than the list holds.
    truncated: bool,
}

#[tauri::command(async)]
pub fn files_list(state: State<'_, AppState>, pane: Option<Id>) -> Result<FileList, String> {
    let root = pane
        .and_then(|p| state.runtime.live_cwd(&p))
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .or_else(dirs_home)
        .ok_or("no folder to list")?;
    let mut files = git_files(&root).unwrap_or_else(|| walk(&root));
    let truncated = files.len() > MAX_FILES;
    files.truncate(MAX_FILES);
    Ok(FileList {
        root: root.display().to_string(),
        files,
        truncated,
    })
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn git_files(root: &Path) -> Option<Vec<String>> {
    let out = std::process::Command::new("git")
        .args(["ls-files", "-co", "--exclude-standard", "-z"])
        .current_dir(root)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    Some(
        out.stdout
            .split(|b| *b == 0)
            .filter(|p| !p.is_empty())
            .filter_map(|p| std::str::from_utf8(p).ok().map(String::from))
            .take(MAX_FILES + 1)
            .collect(),
    )
}

/// Files under `root`, breadth first, hidden entries and `node_modules` and `target` skipped.
fn walk(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut queue = std::collections::VecDeque::from([root.to_path_buf()]);
    while let Some(dir) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || name == "node_modules" || name == "target" {
                continue;
            }
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() {
                queue.push_back(e.path());
            } else if kind.is_file()
                && let Ok(rel) = e.path().strip_prefix(root)
            {
                out.push(rel.to_string_lossy().into_owned());
                if out.len() > MAX_FILES {
                    return out;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_walk_skips_hidden_and_build_folders() {
        let root =
            std::env::temp_dir().join(format!("terminescu-walk-{}", terminescu_core::new_id()));
        for d in ["src", ".git", "node_modules", "target"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
            std::fs::write(root.join(d).join("a.txt"), "x").unwrap();
        }
        std::fs::write(root.join("README.md"), "x").unwrap();
        std::fs::write(root.join(".env"), "x").unwrap();
        assert_eq!(walk(&root), vec!["README.md", "src/a.txt"]);
        std::fs::remove_dir_all(root).unwrap();
    }
}

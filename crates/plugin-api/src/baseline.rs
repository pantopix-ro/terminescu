//! The baseline store (add-claude-insights D2, D3): what files held before an agent session
//! changed them. Shared by the plugin that reads it and by `terminescu hook`, which makes the
//! first-touch copies and knows no agent: the hook's command line names where in the event the
//! session and the path are.
//!
//! Layout, under `<support>/baselines`:
//!
//! ```text
//! <session>/            0700
//!   ready               the working-tree snapshot is done (its tree id, then its root folder)
//!   index, objects/     the snapshot's own git index and object directory
//!   files/<key>.path    the file's absolute path; made first, with O_EXCL, so one hook wins
//!   files/<key>         a clone of the file before the first edit, 0600
//!   files/<key>.new     the file did not exist before the first edit
//!   files/<key>.none    no copy: over the size cap or on another volume (the reason inside)
//! ```

use std::ffi::CString;
use std::fs::{DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The largest file the hook copies and the plugin compares (5 MB, the spec's default).
pub const SIZE_CAP: u64 = 5 * 1024 * 1024;
/// The marker of a finished working-tree snapshot.
pub const READY: &str = "ready";

/// `<support>/baselines`.
pub fn store(support: &Path) -> PathBuf {
    support.join("baselines")
}

/// A session's folder, or `None` for an id that is not a plain session id.
pub fn session_dir(store: &Path, session: &str) -> Option<PathBuf> {
    crate::is_session_id(session).then(|| store.join(session))
}

/// The name a file's copy goes under: two FNV-1a 64-bit hashes of the path, as hex. The
/// `.path` file beside it settles any collision.
pub fn key(path: &Path) -> String {
    let bytes = path.as_os_str().as_bytes();
    let fnv = |seed: u64| {
        bytes.iter().fold(seed, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    };
    format!(
        "{:016x}{:016x}",
        fnv(0xcbf2_9ce4_8422_2325),
        fnv(0x6c62_272e_07bb_0142)
    )
}

/// What [`first_touch`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touch {
    /// A copy, or a marker, was made now.
    Made,
    /// The session already had a copy or marker for the path, or the snapshot is ready and
    /// covers the path.
    Had,
    /// The path looks like a secret ([`is_secret_path`]): nothing was copied.
    Skipped,
}

/// Home folders whose files are never copied, relative to `$HOME`.
const SECRET_DIRS: [&str; 4] = [".ssh", ".aws", ".gnupg", ".config/gh"];

/// Whether `path` looks like a secret: `.env` and `.env.*`, `*.pem`, `*.key`, `*.p12`, `*.pfx`,
/// private keys (`id_*` without `.pub`), `.npmrc`, `.netrc`, `credentials`, and anything under
/// `~/.ssh`, `~/.aws`, `~/.gnupg` or `~/.config/gh`. The hook never copies such a file and the
/// snapshot keeps untracked ones out of the store.
pub fn is_secret_path(path: &Path) -> bool {
    is_secret_path_in(path, std::env::var_os("HOME").as_deref().map(Path::new))
}

/// [`is_secret_path`] with the home folder given.
pub fn is_secret_path_in(path: &Path, home: Option<&Path>) -> bool {
    if let Some(home) = home.filter(|h| h.is_absolute())
        && SECRET_DIRS.iter().any(|d| path.starts_with(home.join(d)))
    {
        return true;
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    if name == ".env"
        || name.starts_with(".env.")
        || matches!(name.as_str(), ".npmrc" | ".netrc" | "credentials")
        || (name.starts_with("id_") && !name.ends_with(".pub"))
    {
        return true;
    }
    matches!(
        Path::new(&name).extension().and_then(|e| e.to_str()),
        Some("pem" | "key" | "p12" | "pfx")
    )
}

/// The folder a ready snapshot covers: the second line of the `ready` marker.
pub fn snapshot_root(dir: &Path) -> Option<PathBuf> {
    let text = std::fs::read(dir.join(READY)).ok()?;
    let line = text.split(|b| *b == b'\n').nth(1)?;
    (!line.is_empty()).then(|| PathBuf::from(std::ffi::OsStr::from_bytes(line)))
}

/// Whether the session's snapshot holds `path`: it is ready and `path` lies under its root
/// (gitignored files under it are not in the snapshot; a known gap, see design.md).
fn snapshot_covers(dir: &Path, path: &Path) -> bool {
    !path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
        && snapshot_root(dir).is_some_and(|root| path.starts_with(root))
}

fn private_dir(dir: &Path) -> std::io::Result<()> {
    DirBuilder::new().recursive(true).mode(0o700).create(dir)
}

fn small_file(path: &Path, text: &str, exclusive: bool) -> std::io::Result<()> {
    let mut o = OpenOptions::new();
    o.write(true).mode(0o600);
    if exclusive {
        o.create_new(true);
    } else {
        o.create(true).truncate(true);
    }
    o.open(path)?.write_all(text.as_bytes())
}

/// Before an agent's first edit of `path` in `session`: an APFS clone of the file, or a marker
/// when it is new, over `cap`, or on another volume. One `stat`, one `clonefile`, no reads of
/// the file's contents. Does nothing for a secret-looking path, nor once the session's snapshot
/// is ready and covers the path (it lies under the snapshot's root).
pub fn first_touch(store: &Path, session: &str, path: &Path, cap: u64) -> std::io::Result<Touch> {
    let Some(dir) = session_dir(store, session) else {
        return Err(std::io::Error::other("not a session id"));
    };
    if !path.is_absolute() {
        return Err(std::io::Error::other("not an absolute path"));
    }
    if is_secret_path(path) {
        return Ok(Touch::Skipped);
    }
    if snapshot_covers(&dir, path) {
        return Ok(Touch::Had);
    }
    let files = dir.join("files");
    private_dir(&files)?;
    let key = key(path);
    let mut text = path.as_os_str().as_bytes().to_vec();
    text.push(b'\n');
    // The claim: whoever creates `.path` makes the copy; a second hook for the path stops here.
    match small_file(
        &files.join(format!("{key}.path")),
        &String::from_utf8_lossy(&text),
        true,
    ) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Ok(Touch::Had),
        Err(e) => return Err(e),
    }
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            small_file(&files.join(format!("{key}.new")), "", false)?;
            return Ok(Touch::Made);
        }
        Err(e) => return Err(e),
    };
    if !meta.is_file() {
        small_file(&files.join(format!("{key}.none")), "not a file\n", false)?;
        return Ok(Touch::Made);
    }
    if meta.len() > cap {
        small_file(&files.join(format!("{key}.none")), "too large\n", false)?;
        return Ok(Touch::Made);
    }
    let copy = files.join(&key);
    match clone(path, &copy) {
        Ok(()) => {
            std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o600))?;
            Ok(Touch::Made)
        }
        Err(_) => {
            // Another volume, or a file system without clones: a marker, never a slow copy.
            small_file(&files.join(format!("{key}.none")), "not cloned\n", false)?;
            Ok(Touch::Made)
        }
    }
}

#[cfg(target_os = "macos")]
fn clone(from: &Path, to: &Path) -> std::io::Result<()> {
    let from = CString::new(from.as_os_str().as_bytes())?;
    let to = CString::new(to.as_os_str().as_bytes())?;
    // SAFETY: both are valid NUL-terminated paths; clonefile(2) reads them and nothing else.
    let rc = unsafe { libc::clonefile(from.as_ptr(), to.as_ptr(), 0) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "macos"))]
fn clone(_from: &Path, _to: &Path) -> std::io::Result<()> {
    let _ = CString::default();
    Err(std::io::Error::other("clonefile is macOS only"))
}

/// A first-touch copy as the plugin reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Copy {
    /// The bytes before the first edit.
    Before(PathBuf),
    /// The file did not exist.
    New,
    /// No copy; the reason.
    None(String),
}

/// Every first-touch copy of a session, by absolute path.
pub fn copies(store: &Path, session: &str) -> Vec<(PathBuf, Copy)> {
    let Some(files) = session_dir(store, session).map(|d| d.join("files")) else {
        return Vec::new();
    };
    let Ok(read) = std::fs::read_dir(&files) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in read.flatten() {
        let name = entry.file_name();
        let Some(key) = name.to_str().and_then(|n| n.strip_suffix(".path")) else {
            continue;
        };
        let Ok(text) = std::fs::read(entry.path()) else {
            continue;
        };
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(
            text.strip_suffix(b"\n").unwrap_or(&text),
        ));
        if self::key(&path) != key {
            continue;
        }
        let copy = files.join(key);
        let kind = if copy.is_file() {
            Copy::Before(copy)
        } else if files.join(format!("{key}.new")).exists() {
            Copy::New
        } else if let Ok(why) = std::fs::read_to_string(files.join(format!("{key}.none"))) {
            Copy::None(why.trim().to_string())
        } else {
            // The claim exists but the hook stopped before the copy: nothing to compare with.
            Copy::None("not copied".into())
        };
        out.push((path, kind));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tsc-baseline-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn first_touch_copies_once() {
        let dir = temp("once");
        let store = dir.join("store");
        let file = dir.join("todo.md");
        std::fs::write(&file, "before\n").unwrap();
        assert_eq!(
            first_touch(&store, "s-1", &file, SIZE_CAP).unwrap(),
            Touch::Made
        );
        std::fs::write(&file, "after\n").unwrap();
        assert_eq!(
            first_touch(&store, "s-1", &file, SIZE_CAP).unwrap(),
            Touch::Had
        );
        let got = copies(&store, "s-1");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, file);
        match &got[0].1 {
            Copy::Before(p) => {
                assert_eq!(std::fs::read_to_string(p).unwrap(), "before\n");
                let mode = std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
                assert_eq!(mode, 0o600);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_large_file_gets_a_marker() {
        let dir = temp("large");
        let store = dir.join("store");
        let file = dir.join("big.bin");
        std::fs::write(&file, vec![b'x'; 64]).unwrap();
        first_touch(&store, "s-1", &file, 10).unwrap();
        assert_eq!(copies(&store, "s-1")[0].1, Copy::None("too large".into()));
    }

    #[test]
    fn a_new_file_is_marked_new_and_a_ready_snapshot_stops_copies() {
        let dir = temp("new");
        let store = dir.join("store");
        first_touch(&store, "s-1", &dir.join("fresh.md"), SIZE_CAP).unwrap();
        assert_eq!(copies(&store, "s-1")[0].1, Copy::New);
        std::fs::write(
            store.join("s-1").join(READY),
            format!("tree\n{}\n", dir.join("repo").display()),
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("repo")).unwrap();
        let inside = dir.join("repo").join("other.md");
        std::fs::write(&inside, "x").unwrap();
        assert_eq!(
            first_touch(&store, "s-1", &inside, SIZE_CAP).unwrap(),
            Touch::Had
        );
        assert_eq!(copies(&store, "s-1").len(), 1);
    }

    #[test]
    fn a_ready_snapshot_does_not_cover_files_outside_its_root() {
        let dir = temp("outside");
        let store = dir.join("store");
        std::fs::create_dir_all(store.join("s-1")).unwrap();
        std::fs::write(
            store.join("s-1").join(READY),
            format!("tree\n{}\n", dir.join("repo").display()),
        )
        .unwrap();
        let notes = dir.join("notes").join("x.md");
        std::fs::create_dir_all(notes.parent().unwrap()).unwrap();
        std::fs::write(&notes, "before\n").unwrap();
        // A sibling whose name starts like the root, and a dotdot escape, are outside too.
        let sibling = dir.join("repo2").join("y.md");
        std::fs::create_dir_all(sibling.parent().unwrap()).unwrap();
        std::fs::write(&sibling, "y\n").unwrap();
        let escape = dir.join("repo").join("..").join("notes").join("x.md");
        for p in [&notes, &sibling] {
            assert_eq!(
                first_touch(&store, "s-1", p, SIZE_CAP).unwrap(),
                Touch::Made
            );
        }
        assert_eq!(
            first_touch(&store, "s-1", &escape, SIZE_CAP).unwrap(),
            Touch::Made
        );
        assert_eq!(copies(&store, "s-1").len(), 3);
    }

    #[test]
    fn secret_looking_paths_are_recognised() {
        let home = Path::new("/Users/owner");
        for p in [
            "/w/.env",
            "/w/.env.local",
            "/w/tls/server.PEM",
            "/w/a.key",
            "/w/a.p12",
            "/w/a.pfx",
            "/w/id_rsa",
            "/w/id_ed25519",
            "/w/id_custom",
            "/w/.npmrc",
            "/w/.netrc",
            "/w/credentials",
            "/Users/owner/.ssh/config",
            "/Users/owner/.aws/anything/deep",
            "/Users/owner/.gnupg/pubring.kbx",
            "/Users/owner/.config/gh/hosts.yml",
        ] {
            assert!(is_secret_path_in(Path::new(p), Some(home)), "{p}");
        }
        for p in [
            "/w/id_rsa.pub",
            "/w/env.md",
            "/w/.envrc",
            "/w/src/key.ts",
            "/w/identity.ts",
            "/w/credentials.md",
            "/Users/owner/.config/ghost/x",
            "/Users/owner/.sshx/x",
            "/Users/owner/notes/x.md",
        ] {
            assert!(!is_secret_path_in(Path::new(p), Some(home)), "{p}");
        }
        assert!(!is_secret_path_in(Path::new("/Users/owner/.ssh/x"), None));
    }

    #[test]
    fn a_secret_file_is_never_copied() {
        let dir = temp("secret");
        let store = dir.join("store");
        let file = dir.join(".env.local");
        std::fs::write(&file, "TOKEN=x\n").unwrap();
        assert_eq!(
            first_touch(&store, "s-1", &file, SIZE_CAP).unwrap(),
            Touch::Skipped
        );
        assert!(copies(&store, "s-1").is_empty());
    }

    #[test]
    fn odd_sessions_and_relative_paths_are_refused() {
        let dir = temp("odd");
        let store = dir.join("store");
        assert!(first_touch(&store, "../x", &dir.join("a"), SIZE_CAP).is_err());
        assert!(first_touch(&store, "s-1", Path::new("a.md"), SIZE_CAP).is_err());
        assert!(!store.join("..").join("x").exists());
    }
}

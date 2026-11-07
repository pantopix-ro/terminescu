//! Help › Uninstall Terminescu Helpers (add-public-release D9): the files Terminescu put outside
//! its bundle. The hook comes out first (`hook_install::uninstall`), so a later failure never
//! leaves Claude Code calling a missing program; the person's data goes only on request.

use std::io;
use std::path::{Path, PathBuf};

/// Where the helpers and the data live.
pub struct Places {
    /// The support folder: `bin/`, `run/` and `logs/`.
    pub support: PathBuf,
    /// The sockets' folder, which may sit in `/tmp` when the support path is long.
    pub run: PathBuf,
    /// The app's own folders: the database, the WebKit data and caches.
    pub data: Vec<PathBuf>,
}

/// Removes the copied binaries and the sockets' folder, then, with `delete_data`, the support
/// folder and the app's data. A path that is already gone counts as removed.
pub fn helpers(places: &Places, delete_data: bool) -> io::Result<()> {
    remove(&places.support.join("bin"))?;
    remove(&places.run)?;
    if delete_data {
        remove(&places.support)?;
        for d in &places.data {
            remove(d)?;
        }
    }
    Ok(())
}

fn remove(path: &Path) -> io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn places() -> Places {
        let root = std::env::temp_dir().join(format!("terminescu-uninstall-{}", crate::new_id()));
        let support = root.join("Terminescu");
        for d in ["bin", "run", "logs"] {
            std::fs::create_dir_all(support.join(d)).unwrap();
        }
        std::fs::write(support.join("bin/terminescu"), "cli").unwrap();
        std::fs::write(support.join("logs/app.log"), "log").unwrap();
        let data = root.join("ro.example.terminescu");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(data.join("terminescu.sqlite"), "db").unwrap();
        Places {
            run: support.join("run"),
            support,
            data: vec![data, root.join("never-made")],
        }
    }

    #[test]
    fn uninstall_removes_helpers_keeps_db() {
        let p = places();
        helpers(&p, false).unwrap();
        assert!(!p.support.join("bin").exists());
        assert!(!p.run.exists());
        assert!(p.support.join("logs/app.log").exists());
        assert!(p.data[0].join("terminescu.sqlite").exists());
        helpers(&p, false).unwrap();
    }

    #[test]
    fn uninstall_with_data_removes_everything() {
        let p = places();
        helpers(&p, true).unwrap();
        assert!(!p.support.exists());
        assert!(!p.data[0].exists());
    }
}

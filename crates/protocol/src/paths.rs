//! Where the sockets live (D32, D36). The app, the daemon and the CLI all call these
//! functions, so they agree on every path.

use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub const PANE_ID_ENV: &str = "TERMINESCU_PANE_ID";
pub const PTYD_SOCKET_ENV: &str = "TERMINESCU_PTYD_SOCKET";
pub const CORE_SOCKET_ENV: &str = "TERMINESCU_CORE_SOCKET";
/// Overrides the support folder, for tests and for running two builds side by side.
pub const HOME_ENV: &str = "TERMINESCU_HOME";

/// `sun_path` holds 104 bytes on macOS, the terminating NUL included.
pub const SOCKET_PATH_MAX: usize = 103;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunPaths {
    pub dir: PathBuf,
    pub ptyd_socket: PathBuf,
    pub ptyd_lock: PathBuf,
    pub core_socket: PathBuf,
}

/// `~/Library/Application Support/Terminescu`, or `$TERMINESCU_HOME`.
pub fn support_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(HOME_ENV) {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join("Library/Application Support/Terminescu")
}

/// The folder for copied binaries: the daemon and the CLI (D32, D36).
pub fn bin_dir() -> PathBuf {
    support_dir().join("bin")
}

pub fn run_paths() -> RunPaths {
    run_paths_in(&support_dir().join("run"), uid())
}

/// The sockets in `preferred`, or in `/tmp/terminescu-<uid>` when a path would pass the limit.
pub fn run_paths_in(preferred: &Path, uid: u32) -> RunPaths {
    let longest = preferred.join("core.sock").as_os_str().len().max(preferred.join("ptyd.sock").as_os_str().len());
    let dir = if longest <= SOCKET_PATH_MAX { preferred.to_path_buf() } else { PathBuf::from(format!("/tmp/terminescu-{uid}")) };
    RunPaths {
        ptyd_socket: dir.join("ptyd.sock"),
        ptyd_lock: dir.join("ptyd.lock"),
        core_socket: dir.join("core.sock"),
        dir,
    }
}

pub fn uid() -> u32 {
    // SAFETY: getuid has no preconditions and cannot fail.
    unsafe { libc::getuid() }
}

/// Creates `dir` with mode 0700 and checks that the current user owns it, so no other user can
/// plant or read a socket there.
pub fn ensure_private_dir(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let meta = std::fs::metadata(dir)?;
    if meta.uid() != uid() {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, format!("{} belongs to another user", dir.display())));
    }
    if meta.permissions().mode() & 0o777 != 0o700 {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_paths_stay_in_the_support_folder() {
        let p = run_paths_in(Path::new("/Users/owner/Library/Application Support/Terminescu/run"), 501);
        assert_eq!(p.ptyd_socket, Path::new("/Users/owner/Library/Application Support/Terminescu/run/ptyd.sock"));
    }

    #[test]
    fn long_paths_fall_back_to_tmp() {
        let long = format!("/Users/{}/Library/Application Support/Terminescu/run", "o".repeat(60));
        let p = run_paths_in(Path::new(&long), 501);
        assert_eq!(p.dir, Path::new("/tmp/terminescu-501"));
        assert!(p.core_socket.as_os_str().len() <= SOCKET_PATH_MAX);
    }

    #[test]
    fn private_dir_gets_mode_0700() {
        let dir = std::env::temp_dir().join(format!("terminescu-paths-{}", std::process::id()));
        ensure_private_dir(&dir).unwrap();
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

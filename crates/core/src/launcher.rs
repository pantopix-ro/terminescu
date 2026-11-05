//! Installs and starts the pty daemon (D32).
//!
//! The daemon runs from its own copy under the support folder, named after its version and the
//! first 8 hex digits of its SHA-256, so an app update never rewrites a binary that a running
//! daemon still pages in. It starts in its own session with `setsid` and outlives the app.

use sha2::{Digest, Sha256};
use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Copies `bundled` into `bin_dir` as `ptyd-<version>-<hash8>` and returns the copy's path.
/// An existing copy with that name is reused as is.
pub fn install(bundled: &Path, bin_dir: &Path) -> io::Result<PathBuf> {
    let bytes = std::fs::read(bundled)?;
    let hash = Sha256::digest(&bytes);
    let hex: String = hash[..4].iter().map(|b| format!("{b:02x}")).collect();
    let target = bin_dir.join(format!("ptyd-{}-{hex}", env!("CARGO_PKG_VERSION")));
    if target.exists() {
        return Ok(target);
    }
    std::fs::create_dir_all(bin_dir)?;
    // Write under a temporary name, then rename: the rename is atomic, so no process ever
    // sees a half-written binary.
    let tmp = bin_dir.join(format!(".ptyd-{}-{hex}.tmp", std::process::id()));
    std::fs::write(&tmp, &bytes)?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    clear_quarantine(&tmp)?;
    std::fs::rename(&tmp, &target)?;
    Ok(target)
}

/// Copies the CLI to `bin_dir/terminescu` when its content changed (D36). The rename leaves a
/// running `terminescu` on the old file, so a hook in flight never breaks.
pub fn install_cli(bundled: &Path, bin_dir: &Path) -> io::Result<PathBuf> {
    let bytes = std::fs::read(bundled)?;
    let target = bin_dir.join("terminescu");
    if std::fs::read(&target).is_ok_and(|old| old == bytes) {
        return Ok(target);
    }
    std::fs::create_dir_all(bin_dir)?;
    let tmp = bin_dir.join(format!(".terminescu-{}.tmp", std::process::id()));
    std::fs::write(&tmp, &bytes)?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    clear_quarantine(&tmp)?;
    std::fs::rename(&tmp, &target)?;
    Ok(target)
}

/// Removes `com.apple.quarantine`, which Gatekeeper would otherwise check on the copy.
fn clear_quarantine(path: &Path) -> io::Result<()> {
    let p = CString::new(path.as_os_str().as_bytes()).map_err(io::Error::other)?;
    let name = c"com.apple.quarantine";
    // SAFETY: both pointers are valid NUL-terminated strings.
    if unsafe { libc::removexattr(p.as_ptr(), name.as_ptr(), 0) } != 0 {
        let err = io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::ENOATTR) {
            return Err(err);
        }
    }
    Ok(())
}

/// True when a daemon accepts connections on `socket`.
pub fn is_running(socket: &Path) -> bool {
    UnixStream::connect(socket).is_ok()
}

/// Starts `daemon` detached unless one already answers on `socket`, then waits until the socket
/// accepts connections. `extra` goes on the daemon's command line after `--socket`.
pub fn ensure_running(
    daemon: &Path,
    socket: &Path,
    log: &Path,
    extra: &[&str],
    timeout: Duration,
) -> io::Result<()> {
    if is_running(socket) {
        return Ok(());
    }
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)?;
    let mut cmd = Command::new(daemon);
    cmd.arg("--socket")
        .arg(socket)
        .args(extra)
        .stdin(Stdio::null())
        .stdout(log_file.try_clone()?)
        .stderr(log_file);
    // SAFETY: setsid is async-signal-safe and touches no Rust state.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = cmd.spawn()?;
    let start = Instant::now();
    loop {
        if is_running(socket) {
            // The daemon keeps running; reap it later only if it exits early.
            std::thread::spawn(move || child.wait());
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            // It lost the lock to a daemon that started at the same moment, or failed.
            if is_running(socket) {
                return Ok(());
            }
            return Err(io::Error::other(format!(
                "daemon exited with {status}; see {}",
                log.display()
            )));
        }
        if start.elapsed() > timeout {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "daemon socket did not appear",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_names_the_copy_by_hash_and_reuses_it() {
        let dir = std::env::temp_dir().join(format!("tlaunch-{}", std::process::id()));
        let src = dir.join("src-bin");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&src, b"#!/bin/sh\necho one\n").unwrap();
        let bin = dir.join("bin");
        let a = install(&src, &bin).unwrap();
        let b = install(&src, &bin).unwrap();
        assert_eq!(a, b);
        let name = a.file_name().unwrap().to_str().unwrap().to_owned();
        assert!(
            name.starts_with(&format!("ptyd-{}-", env!("CARGO_PKG_VERSION")))
                && name.len() == "ptyd-".len() + env!("CARGO_PKG_VERSION").len() + 9,
            "{name}"
        );
        assert_eq!(
            std::fs::metadata(&a).unwrap().permissions().mode() & 0o777,
            0o755
        );

        // A new build gets a new name; the old copy stays for the daemon that runs it.
        std::fs::write(&src, b"#!/bin/sh\necho two\n").unwrap();
        let c = install(&src, &bin).unwrap();
        assert_ne!(a, c);
        assert!(a.exists());
        let leftovers: Vec<_> = std::fs::read_dir(&bin)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

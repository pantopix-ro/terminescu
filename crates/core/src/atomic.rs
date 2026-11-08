//! Writing a file the person owns without ever leaving it half written.

use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Writes `bytes` to `path` through a temp file in the same folder and a rename, so a full
/// disk or a crash leaves the old file whole. A symlink is followed and its target replaced;
/// the file keeps its permissions, and the temp file is never readable by more people than
/// the file itself.
pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dir = target.parent().unwrap_or(Path::new("."));
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp: PathBuf = dir.join(format!(".{name}.terminescu-{}", std::process::id()));
    let mode = std::fs::metadata(&target).map_or(0o644, |m| m.permissions().mode() & 0o7777);
    let _ = std::fs::remove_file(&tmp);
    let written = (|| {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode & 0o600)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode))?;
        std::fs::rename(&tmp, &target)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rewrite_keeps_the_mode_and_follows_a_symlink() {
        let dir = std::env::temp_dir().join(format!("terminescu-atomic-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let real = dir.join("real.json");
        std::fs::write(&real, "old").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = dir.join("link.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        write(&link, b"new").unwrap();
        assert_eq!(std::fs::read_to_string(&real).unwrap(), "new");
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            std::fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // No temp file is left behind.
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);

        write(&dir.join("fresh.txt"), b"x").unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("fresh.txt")).unwrap(), "x");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

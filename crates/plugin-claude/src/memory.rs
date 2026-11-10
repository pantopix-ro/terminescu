//! Claude's memory files and transcript folder (add-claude-insights 3.3). Pure path logic: the
//! plugin opens the files through `PluginCx::open_text` and shows the folder through
//! `PluginCx::reveal`.

use std::path::{Path, PathBuf};

/// Claude Code's own folder: `$CLAUDE_CONFIG_DIR`, else `~/.claude`.
pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(".claude")
}

/// The project's `CLAUDE.md`: at the repository root above `cwd` (the nearest folder holding
/// `.git`, a folder or a worktree's file), else in `cwd` itself.
pub fn project_file(cwd: &Path) -> PathBuf {
    cwd.ancestors()
        .find(|d| d.join(".git").exists())
        .unwrap_or(cwd)
        .join("CLAUDE.md")
}

/// The person's own `CLAUDE.md`, read by every session.
pub fn user_file(config: &Path) -> PathBuf {
    config.join("CLAUDE.md")
}

/// The folder Claude Code keeps a project's transcripts in: the cwd with every character
/// outside `[A-Za-z0-9]` turned into `-`.
pub fn project_dir(config: &Path, cwd: &Path) -> PathBuf {
    let name: String = cwd
        .display()
        .to_string()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    config.join("projects").join(name)
}

/// What "Reveal Transcript Folder" shows: the newest transcript found (selected in its
/// folder), else the cwd's project folder, else the projects folder. None when Claude never
/// wrote one.
pub fn reveal_target(
    config: &Path,
    cwd: Option<&Path>,
    newest: Option<PathBuf>,
) -> Option<PathBuf> {
    newest
        .filter(|p| p.is_file())
        .or_else(|| cwd.map(|c| project_dir(config, c)).filter(|d| d.is_dir()))
        .or_else(|| Some(config.join("projects")).filter(|d| d.is_dir()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_project_file_sits_at_the_repository_root() {
        let base = std::env::temp_dir().join(format!("tsc-memory-{}", std::process::id()));
        let deep = base.join("repo/src/lib");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::create_dir_all(base.join("repo/.git")).unwrap();
        assert_eq!(project_file(&deep), base.join("repo/CLAUDE.md"));
        let plain = base.join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        // `plain` has no repository above it inside the temp folder, unless the temp folder
        // itself is in one; the nearest `.git` wins either way.
        let found = project_file(&plain);
        assert!(found == plain.join("CLAUDE.md") || !found.starts_with(&base));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn a_project_folder_is_named_after_its_cwd() {
        let d = project_dir(Path::new("/c"), Path::new("/Users/owner/my repo.v2"));
        assert_eq!(d, Path::new("/c/projects/-Users-owner-my-repo-v2"));
    }
}

//! Where a worktree goes (add-git-plugin D5): next to the repository as `<repo>-<branch>`, or
//! under a chosen folder as `<folder>/<repo>/<branch>`. A path that exists gets `-2`, `-3`.

use std::path::{Path, PathBuf};

/// The branch as one path part: `/` and anything outside `[A-Za-z0-9._-]` become `-`, and no
/// leading dot is left, so the part never climbs out of its folder.
pub fn slug(branch: &str) -> String {
    let s: String = branch
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_start_matches('.');
    if s.is_empty() {
        "worktree".into()
    } else {
        s.into()
    }
}

/// The worktree for `branch` of the repository whose main working tree is `main_root`.
pub fn path_for(main_root: &Path, repo: &str, branch: &str, folder: Option<&Path>) -> PathBuf {
    let (repo, branch) = (slug(repo), slug(branch));
    let first = match folder {
        Some(f) => f.join(repo).join(branch),
        None => main_root
            .parent()
            .unwrap_or(main_root)
            .join(format!("{repo}-{branch}")),
    };
    if !first.exists() {
        return first;
    }
    let name = first
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    (2..)
        .map(|n| first.with_file_name(format!("{name}-{n}")))
        .find(|p| !p.exists())
        .expect("some suffix is free")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_stay_one_safe_path_part() {
        assert_eq!(slug("feature/PROJ-482-retry"), "feature-PROJ-482-retry");
        assert_eq!(slug("fix: a b"), "fix--a-b");
        assert_eq!(slug("v1.2_rc"), "v1.2_rc");
        assert_eq!(slug(".."), "worktree");
        assert_eq!(slug(".hidden"), "hidden");
        assert_eq!(slug("ü"), "-");
    }
}

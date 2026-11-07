//! Where the public project lives (add-public-release D4, D7). Every link to the repository,
//! the update feed and the feedback address comes from here, so a move changes one file.

/// The public GitHub repository, as `owner/name`.
pub const PUBLIC_REPO: &str = "pantopix/terminescu";
/// The maker named in About.
pub const MAKER: &str = "PANTOPIX RO";
pub const MAKER_URL: &str = "https://pantopix.ro";
/// Feedback mail goes here, with a subject that starts with `FEEDBACK_PREFIX` for filtering.
pub const FEEDBACK_EMAIL: &str = "salut@pantopix.com";
pub const FEEDBACK_PREFIX: &str = "[Terminescu]";

/// The repository's web page.
pub fn repo_url() -> String {
    format!("https://github.com/{PUBLIC_REPO}")
}

/// The updater's feed: the newest release's `latest.json`.
pub fn update_feed() -> String {
    format!("{}/releases/latest/download/latest.json", repo_url())
}

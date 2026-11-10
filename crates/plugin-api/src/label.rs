//! Short labels that are safe to show where they stay, as Notification Center keeps what it
//! shows (polish-claude-insights D6).

/// A command as its notification names it: the program and a plain subcommand, as `cargo
/// build` or `npm run`. Leading `NAME=value` assignments, flags and arguments stay out, since
/// they may hold a secret and Notification Center keeps what it shows.
pub fn command_label(command: &str) -> Option<String> {
    let mut words = command
        .split_whitespace()
        .skip_while(|w| w.split_once('=').is_some_and(|(k, _)| is_name(k)));
    let program = words.next()?;
    let program = program.rsplit('/').next().filter(|p| is_word(p))?;
    Some(match words.next().filter(|w| is_subcommand(w)) {
        Some(sub) => format!("{program} {sub}"),
        None => program.to_owned(),
    })
}

/// A subcommand as `build`, `run` or `compose`: lowercase letters, `-` and `:`. A token or an
/// id passed as the first argument has digits or capitals and stays out.
fn is_subcommand(s: &str) -> bool {
    (1..=20).contains(&s.len())
        && s.starts_with(|c: char| c.is_ascii_lowercase())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || "-:".contains(c))
}

fn is_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A short plain word: letters first, then letters, digits, `-`, `_`, `.` or `:`.
fn is_word(s: &str) -> bool {
    s.len() <= 32
        && s.starts_with(|c: char| c.is_ascii_alphabetic())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.:".contains(c))
}

/// A URL without its query and fragment, which may hold a token.
pub fn url_label(url: &str) -> &str {
    let end = url.find(['?', '#']).unwrap_or(url.len());
    &url[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_notification_names_the_program_not_its_arguments() {
        assert_eq!(
            command_label("cargo build --release").as_deref(),
            Some("cargo build")
        );
        assert_eq!(command_label("npm run dev").as_deref(), Some("npm run"));
        assert_eq!(
            command_label("API_TOKEN=abc123 ./deploy.sh prod").as_deref(),
            Some("deploy.sh prod")
        );
        assert_eq!(
            command_label("curl -H 'Authorization: Bearer x' https://example.com").as_deref(),
            Some("curl")
        );
        assert_eq!(command_label("make").as_deref(), Some("make"));
        // A bare token as the first argument is not a subcommand.
        assert_eq!(command_label("echo tok_AbC123xyz").as_deref(), Some("echo"));
        assert_eq!(
            command_label("vault login hvs.CAESxyz").as_deref(),
            Some("vault login")
        );
        assert_eq!(command_label("open s3cr3t").as_deref(), Some("open"));
        assert_eq!(command_label("  ").as_deref(), None);
        assert_eq!(command_label("'quoted thing'").as_deref(), None);
    }

    #[test]
    fn a_url_loses_its_query() {
        assert_eq!(url_label("https://x.test/a?token=1#b"), "https://x.test/a");
        assert_eq!(url_label("https://x.test/a"), "https://x.test/a");
    }
}

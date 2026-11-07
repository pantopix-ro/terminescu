//! The two feedback routes of Help (add-public-release D7): an email to the maker and a new
//! GitHub issue, both filled in with the versions. The app only opens them; the person sends.

use terminescu_protocol::public::{FEEDBACK_EMAIL, FEEDBACK_PREFIX, repo_url};

/// What the reports carry about the Mac and the build.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    /// The commit the build came from, or `dev`.
    pub build: String,
    pub macos: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Route {
    Email,
    Issue,
}

fn body(info: &AppInfo, log: bool) -> String {
    let mut b = format!(
        "\n\n---\nTerminescu {} ({})\nmacOS {}\n",
        info.version, info.build, info.macos
    );
    if log {
        b.push_str("app.log: attached by hand\n");
    }
    b
}

/// Percent-encodes everything but the unreserved characters, as RFC 3986 asks of a query.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The link a route opens: a `mailto:` draft whose subject starts with `[Terminescu]`, or a
/// new issue on the public repository.
pub fn link(route: Route, info: &AppInfo, log: bool) -> String {
    let body = encode(&body(info, log));
    match route {
        Route::Email => format!(
            "mailto:{FEEDBACK_EMAIL}?subject={}&body={body}",
            encode(&format!("{FEEDBACK_PREFIX} Feedback"))
        ),
        Route::Issue => format!("{}/issues/new?body={body}", repo_url()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> AppInfo {
        AppInfo {
            version: "0.2.0".into(),
            build: "abc1234".into(),
            macos: "15.4".into(),
        }
    }

    #[test]
    fn feedback_links_carry_versions() {
        let mail = link(Route::Email, &info(), false);
        assert!(
            mail.starts_with("mailto:salut@pantopix.com?subject=%5BTerminescu%5D%20Feedback&body=")
        );
        assert!(mail.contains("Terminescu%200.2.0%20%28abc1234%29"));
        assert!(mail.contains("macOS%2015.4"));
        assert!(!mail.contains("app.log"));

        let issue = link(Route::Issue, &info(), true);
        assert!(issue.starts_with("https://github.com/pantopix/terminescu/issues/new?body="));
        assert!(issue.contains("macOS%2015.4"));
        assert!(issue.contains("app.log%3A%20attached%20by%20hand"));
    }
}

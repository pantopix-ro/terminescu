//! Links in a side tab's context (refine-context-links): one cleaner for every source (D1, D2),
//! names ranked by where they came from (D5), and labels for links without one (D6).

use serde::Serialize;
use url::Url;

/// Tracking parameters the cleaner drops, matched without case. `utm_` is a prefix (D2).
const TRACKING: &[&str] = &[
    "fbclid", "gclid", "dclid", "msclkid", "mc_cid", "mc_eid", "igshid", "_hsenc", "_hsmi",
];

/// Where a link's name came from; a stronger source wins (D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NameRank {
    None = 0,
    /// `[Name](url)` in a reply or in terminal output.
    Text = 1,
    /// The `Artifact` tool call, or a PR title from `gh`.
    Tool = 2,
}

impl NameRank {
    pub fn from_i64(n: i64) -> Self {
        match n {
            2.. => NameRank::Tool,
            1 => NameRank::Text,
            _ => NameRank::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkType {
    Pr,
    Artifact,
    Web,
}

/// The cleaned form of an http(s) link, or `None` when `raw` is not one.
pub fn clean(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.contains(char::is_whitespace) {
        return None;
    }
    let mut url = Url::parse(raw).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none_or(str::is_empty) {
        return None;
    }
    let _ = url.set_username("");
    let _ = url.set_password(None);
    // `Url` already lowercases the host and drops the default port.
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| {
            let k = k.to_ascii_lowercase();
            !k.starts_with("utm_") && !TRACKING.contains(&k.as_str())
        })
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let all = url.query_pairs().count();
    if url.query().is_some() && kept.len() < all {
        if kept.is_empty() {
            url.set_query(None);
        } else {
            url.query_pairs_mut().clear().extend_pairs(kept);
        }
    }
    if let Some(canon) = canonical(&url) {
        return Some(canon);
    }
    let path = url.path().to_string();
    if path.len() > 1 && path.ends_with('/') {
        url.set_path(path.trim_end_matches('/'));
    }
    if url.fragment() == Some("") {
        url.set_fragment(None);
    }
    Some(url.to_string())
}

/// A GitHub PR or claude.ai artifact URL, cut to its base form.
fn canonical(url: &Url) -> Option<String> {
    let seg: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    let id_ok = |id: &str| {
        !id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    match url.host_str()? {
        "github.com" | "www.github.com" => {
            let [owner, repo, "pull", n, ..] = seg.as_slice() else {
                return None;
            };
            let ok = |x: &str| {
                !x.is_empty()
                    && x.chars()
                        .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            };
            (ok(owner) && ok(repo) && !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())).then(
                || {
                    format!(
                        "https://github.com/{}/{}/pull/{n}",
                        owner.to_ascii_lowercase(),
                        repo.to_ascii_lowercase()
                    )
                },
            )
        }
        "claude.ai" => match seg.as_slice() {
            ["artifact", id, ..] if id_ok(id) => Some(format!("https://claude.ai/artifact/{id}")),
            ["public", "artifacts", id, ..] if id_ok(id) => {
                Some(format!("https://claude.ai/public/artifacts/{id}"))
            }
            ["code", "artifact", id, ..] if id_ok(id) => {
                Some(format!("https://claude.ai/code/artifact/{id}"))
            }
            _ => None,
        },
        _ => None,
    }
}

/// What kind of link a cleaned URL is.
pub fn link_type(url: &str) -> LinkType {
    if pr_parts(url).is_some() {
        LinkType::Pr
    } else if url.starts_with("https://claude.ai/") && url.contains("artifact") {
        LinkType::Artifact
    } else {
        LinkType::Web
    }
}

/// Repository and number of a cleaned GitHub PR URL.
pub fn pr_parts(url: &str) -> Option<(&str, u32)> {
    let rest = url.strip_prefix("https://github.com/")?;
    let mut it = rest.split('/');
    let (_, repo, pull, n) = (it.next()?, it.next()?, it.next()?, it.next()?);
    (pull == "pull" && it.next().is_none())
        .then(|| n.parse().ok().map(|n| (repo, n)))
        .flatten()
}

/// The label of a link without a name (D6).
pub fn label(url: &str) -> String {
    match link_type(url) {
        LinkType::Pr => match pr_parts(url) {
            Some((repo, n)) => format!("PR #{n} · {repo}"),
            None => url.into(),
        },
        LinkType::Artifact => "Artifact".into(),
        LinkType::Web => match Url::parse(url) {
            Ok(u) => {
                let host = u.host_str().unwrap_or_default();
                let host = host.strip_prefix("www.").unwrap_or(host);
                match u
                    .path_segments()
                    .and_then(|mut s| s.rfind(|s| !s.is_empty()))
                {
                    Some(last) => format!("{host} · {last}"),
                    None => host.to_string(),
                }
            }
            Err(_) => url.into(),
        },
    }
}

/// Longest name kept, in characters.
pub const MAX_NAME: usize = 120;

/// A name fit to show: trimmed, without control or bidi characters, cut to [`MAX_NAME`].
/// `None` when nothing is left or the text is a URL or a filler word (D5).
pub fn tidy_name(raw: &str) -> Option<String> {
    let bidi = |c: char| matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
    let text: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .filter(|c| !bidi(*c))
        .collect();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = text.to_lowercase();
    if text.is_empty()
        || matches!(lower.as_str(), "here" | "link" | "this" | "url")
        || lower.starts_with("http://")
        || lower.starts_with("https://")
    {
        return None;
    }
    Some(if text.chars().count() > MAX_NAME {
        let mut cut: String = text.chars().take(MAX_NAME - 1).collect();
        cut.push('…');
        cut
    } else {
        text
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_tracking_case_and_port_go() {
        assert_eq!(
            clean("https://me:tok@Docs.Example.test:443/guide/?utm_source=mail&page=2#setup")
                .as_deref(),
            Some("https://docs.example.test/guide?page=2#setup")
        );
        assert_eq!(
            clean("http://example.test:80/a?fbclid=1&GCLID=2").as_deref(),
            Some("http://example.test/a")
        );
    }

    #[test]
    fn root_slash_and_other_query_stay() {
        assert_eq!(
            clean("https://example.test").as_deref(),
            Some("https://example.test/")
        );
        assert_eq!(
            clean("https://example.test/?q=a+b").as_deref(),
            Some("https://example.test/?q=a+b")
        );
    }

    #[test]
    fn untouched_query_keeps_its_encoding() {
        assert_eq!(
            clean("https://example.test/s?path=%2Fa%2Fb&x=%7E").as_deref(),
            Some("https://example.test/s?path=%2Fa%2Fb&x=%7E")
        );
    }

    #[test]
    fn secret_parameters_stay() {
        // Owner, 2026-11-19 (D2): signed links keep working.
        assert_eq!(
            clean("https://files.example.test/a.zip?sig=abc&X-Amz-Signature=def").as_deref(),
            Some("https://files.example.test/a.zip?sig=abc&X-Amz-Signature=def")
        );
    }

    #[test]
    fn pr_and_artifact_forms() {
        for raw in [
            "https://github.com/acme/shop-api/pull/918/files#diff-3a",
            "https://github.com/Acme/Shop-API/pull/918#issuecomment-1",
            "https://www.github.com/acme/shop-api/pull/918/commits?utm_source=x",
        ] {
            assert_eq!(
                clean(raw).as_deref(),
                Some("https://github.com/acme/shop-api/pull/918"),
                "{raw}"
            );
        }
        assert_eq!(
            clean("https://claude.ai/code/artifact/abc-123?tab=1#x").as_deref(),
            Some("https://claude.ai/code/artifact/abc-123")
        );
        assert_eq!(
            clean("https://claude.ai/public/artifacts/abc_1/").as_deref(),
            Some("https://claude.ai/public/artifacts/abc_1")
        );
        // An issue is not a PR: an ordinary link.
        assert_eq!(
            clean("https://github.com/acme/shop-api/issues/7/").as_deref(),
            Some("https://github.com/acme/shop-api/issues/7")
        );
    }

    #[test]
    fn not_links() {
        for raw in [
            "ftp://files.example.test/a",
            "not a link",
            "https://",
            "mailto:a@b.test",
        ] {
            assert_eq!(clean(raw), None, "{raw}");
        }
    }

    #[test]
    fn idn_host() {
        assert_eq!(
            clean("https://BÜCHER.example/a").as_deref(),
            Some("https://xn--bcher-kva.example/a")
        );
    }

    #[test]
    fn labels() {
        assert_eq!(
            label("https://github.com/acme/shop-api/pull/918"),
            "PR #918 · shop-api"
        );
        assert_eq!(label("https://claude.ai/code/artifact/abc"), "Artifact");
        assert_eq!(
            label("https://docs.example.test/guide?page=2"),
            "docs.example.test · guide"
        );
        assert_eq!(label("https://www.example.test/"), "example.test");
        assert_eq!(
            link_type("https://github.com/acme/shop-api/issues/7"),
            LinkType::Web
        );
    }

    #[test]
    fn names() {
        assert_eq!(
            tidy_name("  Release\u{202e} checklist\n").as_deref(),
            Some("Release checklist")
        );
        for filler in ["here", "Link", " this ", "https://example.test/a", ""] {
            assert_eq!(tidy_name(filler), None, "{filler}");
        }
        assert_eq!(
            tidy_name(&"a".repeat(200)).unwrap().chars().count(),
            MAX_NAME
        );
    }
}

//! PR links in a side tab's context (refine-context-links D9): the git plugin names them with
//! the PR title and marks them with the PR's state. Lookups run on their own thread; a timer
//! carries the answers back to the context, as pull request chips do.

use super::{GitPlugin, lock};
use crate::forge::{Checks, ForgeError, LinkedPr, PrState};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use terminescu_plugin_api::{LinkBadge, PluginCx, Tone};

/// How long a PR link's answer stands before a selected side tab asks again.
pub const LINK_FRESH: Duration = Duration::from_secs(120);
/// The timer that carries finished lookups to the context.
pub(super) const LINK_READY: &str = "linkready";

#[derive(Default)]
pub(super) struct LinkPrs {
    known: HashMap<String, (Instant, Option<LinkedPr>)>,
    running: HashSet<String>,
    /// Side tabs waiting for a URL's answer.
    waiting: HashMap<String, HashSet<String>>,
    lost: Option<String>,
}

fn is_pr(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://github.com/") else {
        return false;
    };
    let parts: Vec<&str> = rest.split('/').collect();
    matches!(parts.as_slice(), [_, _, "pull", n] if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The row badge for a PR: its state, coloured by its checks while open.
pub fn badge(pr: &LinkedPr) -> LinkBadge {
    let (text, tone) = match pr.state {
        PrState::Merged => ("merged", Tone::Good),
        PrState::Closed => ("closed", Tone::Neutral),
        PrState::Draft => ("draft", Tone::Neutral),
        PrState::Open => (
            "open",
            match pr.checks {
                Checks::Failing(_) => Tone::Bad,
                Checks::Running => Tone::Warn,
                Checks::Passing => Tone::Good,
                Checks::None => Tone::Neutral,
            },
        ),
    };
    let checks = match &pr.checks {
        Checks::None => "No checks".to_string(),
        Checks::Passing => "Checks pass".to_string(),
        Checks::Running => "Checks running".to_string(),
        Checks::Failing(names) => format!("Checks failing: {}", names.join(", ")),
    };
    LinkBadge {
        text: text.into(),
        tone,
        tip: Some(format!("PR #{} · {checks}", pr.number)),
    }
}

impl GitPlugin {
    /// A new link landed: look a PR link up.
    pub(super) fn link_added(&self, side_tab: &str, url: &str, cx: &dyn PluginCx) {
        if is_pr(url) {
            self.look_up_link(side_tab, url, cx);
        }
    }

    /// A side tab was selected: its PR links whose answer is older than [`LINK_FRESH`] are
    /// looked up again.
    pub(super) fn links_selected(&self, side_tab: &str, cx: &dyn PluginCx) {
        for url in cx.links(side_tab).into_iter().filter(|u| is_pr(u)) {
            self.look_up_link(side_tab, &url, cx);
        }
    }

    fn look_up_link(&self, side_tab: &str, url: &str, cx: &dyn PluginCx) {
        if !lock(&self.state).github {
            // No badge while GitHub is off; a name already given stays.
            let _ = cx.describe_link(side_tab, url, None, None);
            return;
        }
        let Some(forge) = self.forge.clone() else {
            return;
        };
        let mut links = lock(&self.links);
        let fresh = links
            .known
            .get(url)
            .filter(|(at, _)| at.elapsed() < LINK_FRESH)
            .map(|(_, pr)| pr.clone());
        if let Some(pr) = fresh {
            drop(links);
            describe(side_tab, url, pr.as_ref(), cx);
            return;
        }
        links
            .waiting
            .entry(url.to_string())
            .or_default()
            .insert(side_tab.to_string());
        if !links.running.insert(url.to_string()) {
            drop(links);
            cx.schedule(super::PR_POLL, LINK_READY);
            return;
        }
        drop(links);
        let shared: Arc<Mutex<LinkPrs>> = self.links.clone();
        let owned = url.to_string();
        let started = std::thread::Builder::new()
            .name("git-pr-link".into())
            .spawn(move || {
                // Leaves `running` even when `gh` handling panics, or the link waits forever.
                let mut guard = Running(shared.clone(), Some(owned.clone()));
                let found = forge.pr_at(&owned);
                let mut links = lock(&shared);
                links.running.remove(&owned);
                guard.1 = None;
                let pr = match found {
                    Ok(pr) => pr,
                    Err(e @ (ForgeError::NoGh | ForgeError::LoggedOut)) => {
                        links.lost = Some(e.to_string());
                        None
                    }
                    Err(e) => {
                        eprintln!("terminescu: pull request link: {e}");
                        links.known.get(&owned).and_then(|(_, pr)| pr.clone())
                    }
                };
                links.known.insert(owned, (Instant::now(), pr));
            });
        match started {
            Ok(_) => cx.schedule(super::PR_POLL, LINK_READY),
            Err(e) => {
                eprintln!("terminescu: pull request link lookup not started: {e}");
                lock(&self.links).running.remove(url);
            }
        }
    }

    /// Lookups may have answered: name and mark the links that waited, or look again shortly.
    pub(super) fn links_ready(&self, cx: &dyn PluginCx) {
        let mut links = lock(&self.links);
        if let Some(why) = links.lost.take() {
            drop(links);
            eprintln!("terminescu: GitHub part off: {why}");
            self.set_github(cx);
            links = lock(&self.links);
        }
        let done: Vec<String> = links
            .waiting
            .keys()
            .filter(|u| !links.running.contains(*u))
            .cloned()
            .collect();
        let mut answers = Vec::new();
        for url in done {
            let tabs = links.waiting.remove(&url).unwrap_or_default();
            let pr = links.known.get(&url).and_then(|(_, pr)| pr.clone());
            answers.push((url, tabs, pr));
        }
        let more = !links.waiting.is_empty();
        drop(links);
        let on = lock(&self.state).github;
        for (url, tabs, pr) in answers {
            for side_tab in tabs {
                if on {
                    describe(&side_tab, &url, pr.as_ref(), cx);
                } else {
                    let _ = cx.describe_link(&side_tab, &url, None, None);
                }
            }
        }
        if more {
            cx.schedule(super::PR_POLL, LINK_READY);
        }
    }
}

/// Clears a lookup's `running` mark when its thread ends without an answer.
struct Running(Arc<Mutex<LinkPrs>>, Option<String>);

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(url) = self.1.take() {
            lock(&self.0).running.remove(&url);
        }
    }
}

fn describe(side_tab: &str, url: &str, pr: Option<&LinkedPr>, cx: &dyn PluginCx) {
    let result = match pr {
        Some(pr) => cx.describe_link(side_tab, url, Some(&pr.title), Some(badge(pr))),
        None => cx.describe_link(side_tab, url, None, None),
    };
    if let Err(e) = result {
        eprintln!("terminescu: describing a pull request link: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_cleaned_pr_url_counts() {
        assert!(is_pr("https://github.com/acme/shop-api/pull/918"));
        assert!(!is_pr("https://github.com/acme/shop-api/pull/918/files"));
        assert!(!is_pr("https://github.com/acme/shop-api/issues/7"));
        assert!(!is_pr("https://gitlab.example/acme/shop-api/pull/918"));
    }

    #[test]
    fn the_badge_follows_state_then_checks() {
        let pr = |state, checks| LinkedPr {
            number: 918,
            title: "Retry".into(),
            state,
            checks,
        };
        let b = badge(&pr(PrState::Open, Checks::Failing(vec!["lint".into()])));
        assert_eq!((b.text.as_str(), b.tone), ("open", Tone::Bad));
        assert_eq!(b.tip.as_deref(), Some("PR #918 · Checks failing: lint"));
        assert_eq!(badge(&pr(PrState::Merged, Checks::Passing)).text, "merged");
        assert_eq!(badge(&pr(PrState::Draft, Checks::None)).tone, Tone::Neutral);
    }
}

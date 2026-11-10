//! The forge (Strategy, add-git-plugin D4): pull requests for a branch, through GitHub's `gh`.
//! `gh` keeps its own login; Terminescu never reads or stores a token.

use crate::process::{self, RunError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "failing", rename_all = "camelCase")]
pub enum Checks {
    /// No checks reported.
    None,
    Passing,
    Running,
    /// The names of the checks that failed.
    Failing(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u32,
    pub title: String,
    pub url: String,
    /// The head branch.
    pub branch: String,
    pub checks: Checks,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ForgeError {
    #[error("gh is not installed")]
    NoGh,
    #[error("gh is not logged in")]
    LoggedOut,
    #[error("gh took longer than its limit")]
    Timeout,
    #[error("{0}")]
    Failed(String),
}

pub trait Forge: Send + Sync {
    /// Ok when the forge can answer: `gh` is there and logged in, or too slow to say.
    fn available(&self) -> Result<(), ForgeError>;
    /// The open pull request whose head is `branch`, with its checks.
    fn pr_for(&self, root: &Path, branch: &str) -> Result<Option<PullRequest>, ForgeError>;
    /// The repository's open pull requests, newest first.
    fn open_prs(&self, root: &Path) -> Result<Vec<PullRequest>, ForgeError>;
    /// Checks pull request `number` out in the working tree at `dir`, on a branch of its own.
    fn checkout(&self, dir: &Path, number: u32) -> Result<(), ForgeError>;
}

#[derive(Debug, Clone)]
pub struct GhCli {
    program: PathBuf,
    limit: Duration,
    checkout_limit: Duration,
    env: Vec<(OsString, OsString)>,
}

impl Default for GhCli {
    fn default() -> Self {
        Self::new("gh")
    }
}

impl GhCli {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            limit: Duration::from_secs(3),
            checkout_limit: crate::cli::CHANGE_LIMIT,
            env: Vec::new(),
        }
    }

    pub fn with_limit(mut self, limit: Duration) -> Self {
        self.limit = limit;
        self
    }

    pub fn with_env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env
            .push((key.as_ref().to_owned(), value.as_ref().to_owned()));
        self
    }

    fn run(&self, dir: &Path, args: &[&str], limit: Duration) -> Result<String, ForgeError> {
        let mut cmd = Command::new(&self.program);
        cmd.current_dir(dir)
            .args(args)
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_NO_UPDATE_NOTIFIER", "1")
            .env("NO_COLOR", "1")
            .env("GIT_TERMINAL_PROMPT", "0");
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        let out = process::run(&mut cmd, limit).map_err(|e| match e {
            RunError::NotFound => ForgeError::NoGh,
            RunError::Timeout => ForgeError::Timeout,
            RunError::Io(e) => ForgeError::Failed(e),
        })?;
        if out.ok {
            return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
        }
        let err = out.stderr;
        if err.contains("gh auth login") || err.contains("not logged in") {
            return Err(ForgeError::LoggedOut);
        }
        Err(ForgeError::Failed(err))
    }
}

const FIELDS: &str = "number,title,url,headRefName,state,statusCheckRollup";

impl Forge for GhCli {
    fn available(&self) -> Result<(), ForgeError> {
        match self.run(Path::new("/"), &["auth", "status"], self.limit) {
            Ok(_) => Ok(()),
            // `gh auth status` fails when no host is logged in.
            Err(ForgeError::Failed(_)) => Err(ForgeError::LoggedOut),
            // A slow network is not a lost login: lookups find out, and turn the part off
            // when `gh` answers that it is gone or logged out.
            Err(ForgeError::Timeout) => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn pr_for(&self, root: &Path, branch: &str) -> Result<Option<PullRequest>, ForgeError> {
        if branch.is_empty() || branch.starts_with('-') {
            return Ok(None);
        }
        let args = [
            "pr", "list", "--state", "open", "--head", branch, "--json", FIELDS,
        ];
        let text = self.run(root, &args, self.limit)?;
        Ok(parse_prs(&text)?.into_iter().next())
    }

    fn open_prs(&self, root: &Path) -> Result<Vec<PullRequest>, ForgeError> {
        let args = [
            "pr", "list", "--state", "open", "--limit", "100", "--json", FIELDS,
        ];
        let text = self.run(root, &args, self.limit)?;
        parse_prs(&text)
    }

    fn checkout(&self, dir: &Path, number: u32) -> Result<(), ForgeError> {
        let n = number.to_string();
        self.run(dir, &["pr", "checkout", &n], self.checkout_limit)
            .map(|_| ())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPr {
    number: u32,
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    head_ref_name: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    status_check_rollup: Vec<Value>,
}

pub fn parse_prs(text: &str) -> Result<Vec<PullRequest>, ForgeError> {
    let raw: Vec<RawPr> =
        serde_json::from_str(text).map_err(|e| ForgeError::Failed(format!("gh answered: {e}")))?;
    Ok(raw
        .into_iter()
        .filter(|p| p.state.is_empty() || p.state == "OPEN")
        .map(|p| PullRequest {
            checks: checks(&p.status_check_rollup),
            number: p.number,
            title: p.title,
            url: p.url,
            branch: p.head_ref_name,
        })
        .collect())
}

/// One state for a pull request's check runs and commit statuses. A failure wins, then
/// anything still running.
pub fn checks(rollup: &[Value]) -> Checks {
    if rollup.is_empty() {
        return Checks::None;
    }
    let mut failing = Vec::new();
    let mut running = false;
    for c in rollup {
        let s = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or_default();
        let name = [s("name"), s("context")]
            .into_iter()
            .find(|n| !n.is_empty())
            .unwrap_or("a check")
            .to_string();
        if s("__typename") == "StatusContext" {
            match s("state") {
                "FAILURE" | "ERROR" => failing.push(name),
                "PENDING" | "EXPECTED" => running = true,
                _ => {}
            }
            continue;
        }
        if s("status") != "COMPLETED" {
            running = true;
            continue;
        }
        if matches!(
            s("conclusion"),
            "FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE"
        ) {
            failing.push(name);
        }
    }
    if !failing.is_empty() {
        Checks::Failing(failing)
    } else if running {
        Checks::Running
    } else {
        Checks::Passing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_failure_wins_then_running_then_passing() {
        let ok = json!({"__typename": "CheckRun", "name": "build", "status": "COMPLETED", "conclusion": "SUCCESS"});
        let skip = json!({"__typename": "CheckRun", "name": "docs", "status": "COMPLETED", "conclusion": "SKIPPED"});
        let run = json!({"__typename": "CheckRun", "name": "e2e", "status": "IN_PROGRESS", "conclusion": ""});
        let bad = json!({"__typename": "CheckRun", "name": "lint", "status": "COMPLETED", "conclusion": "FAILURE"});
        let ci = json!({"__typename": "StatusContext", "context": "ci/legacy", "state": "ERROR"});
        assert_eq!(checks(&[]), Checks::None);
        assert_eq!(checks(&[ok.clone(), skip]), Checks::Passing);
        assert_eq!(checks(&[ok.clone(), run.clone()]), Checks::Running);
        assert_eq!(
            checks(&[ok, run, bad, ci]),
            Checks::Failing(vec!["lint".into(), "ci/legacy".into()])
        );
    }

    #[test]
    fn closed_pull_requests_are_left_out() {
        let text = json!([
            {"number": 918, "title": "Retry", "url": "https://github.com/acme/shop-api/pull/918", "headRefName": "feature/PROJ-482-retry", "state": "OPEN", "statusCheckRollup": []},
            {"number": 900, "title": "Old", "url": "", "headRefName": "x", "state": "MERGED", "statusCheckRollup": []}
        ])
        .to_string();
        let prs = parse_prs(&text).unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].number, 918);
        assert_eq!(prs[0].branch, "feature/PROJ-482-retry");
        assert!(parse_prs("not json").is_err());
    }
}

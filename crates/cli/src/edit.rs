//! `terminescu edit`: opens a file in a text pane of the calling pane's side tab (D27). With
//! `--wait` it returns when the pane returns (`⌘↵`) or closes, which makes it an `EDITOR`.

use crate::common::{core_socket, fail, pane_id, usage_error};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use terminescu_protocol::core::{CoreMessage, CoreReply, CoreRequest, SendError, request};

const TIMEOUT: Duration = Duration::from_secs(5);
/// An editor request lasts as long as the person edits.
const WAIT: Duration = Duration::from_secs(30 * 86_400);

#[derive(Debug, PartialEq)]
struct Args {
    path: String,
    wait: bool,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut path = None;
    let mut wait = false;
    for arg in args {
        match arg.as_str() {
            "--wait" | "-w" => wait = true,
            flag if flag.starts_with('-') && flag.len() > 1 => {
                return Err(format!("unknown option {flag}"));
            }
            text => {
                if path.replace(text.to_string()).is_some() {
                    return Err("edit takes one file".into());
                }
            }
        }
    }
    let path = path.ok_or("edit needs a file")?;
    Ok(Args { path, wait })
}

pub fn run(args: &[String]) -> ExitCode {
    let args = match parse(args) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    // A program that asked for an editor must see the failure and stop.
    let Some(pane) = pane_id() else {
        return fail("not in a Terminescu pane; open the file from a Terminescu tab");
    };
    let path = match std::path::absolute(PathBuf::from(&args.path)) {
        Ok(p) => p.display().to_string(),
        Err(e) => return fail(&format!("{}: {e}", args.path)),
    };
    let message = CoreMessage::new(
        pane,
        CoreRequest::Edit {
            path,
            wait: args.wait,
        },
    );
    let budget = if args.wait { WAIT } else { TIMEOUT };
    match request(&core_socket(), &message, budget) {
        Ok(CoreReply::Ok) => ExitCode::SUCCESS,
        Ok(CoreReply::Error { code, message }) => fail(&format!("{code}: {message}")),
        Err(SendError::NotRunning) => fail("Terminescu is not running"),
        Err(SendError::Closed) if args.wait => fail("Terminescu quit before the edit returned"),
        Err(e) => fail(&format!("edit failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Args, String> {
        parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn parses_edit_args() {
        assert_eq!(
            p(&["notes.md"]).unwrap(),
            Args {
                path: "notes.md".into(),
                wait: false
            }
        );
        assert!(p(&["--wait", ".git/COMMIT_EDITMSG"]).unwrap().wait);
        for bad in [&[][..], &["a", "b"], &["--wait"], &["a", "--bold"]] {
            assert!(p(bad).is_err(), "{bad:?}");
        }
    }
}

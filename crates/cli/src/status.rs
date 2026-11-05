//! `terminescu status`: sets the pane's tab status through the app (D15, D36).

use crate::common::{core_socket, fail, pane_id, parse_duration, usage_error, warn};
use std::process::ExitCode;
use std::time::Duration;
use terminescu_protocol::core::{CoreMessage, CoreReply, CoreRequest, Expiry, SendError, request};

const TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, PartialEq)]
struct Args {
    label: Option<String>,
    color: Option<String>,
    expiry: Expiry,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut label = None;
    let mut color = None;
    let mut expiry = None;
    let mut clear = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--clear" => clear = true,
            "--color" => color = Some(it.next().ok_or("--color needs a color")?.clone()),
            "--for" => {
                let text = it.next().ok_or("--for needs a duration")?;
                let d = parse_duration(text).ok_or(format!("`{text}` is not a duration"))?;
                if expiry
                    .replace(Expiry::After {
                        ms: d.as_millis() as u64,
                    })
                    .is_some()
                {
                    return Err("pick one of --for and --until-exit".into());
                }
            }
            "--until-exit" => {
                if expiry.replace(Expiry::CommandEnd).is_some() {
                    return Err("pick one of --for and --until-exit".into());
                }
            }
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            text => {
                if label.replace(text.to_string()).is_some() {
                    return Err("quote a label that has spaces".into());
                }
            }
        }
    }
    match (clear, &label) {
        (true, Some(_)) => return Err("--clear takes no label".into()),
        (true, None) if color.is_some() || expiry.is_some() => {
            return Err("--clear takes no other option".into());
        }
        (false, None) => return Err("status needs a label or --clear".into()),
        (false, Some(l)) if l.trim().is_empty() => return Err("the label is empty".into()),
        _ => {}
    }
    Ok(Args {
        label,
        color,
        expiry: expiry.unwrap_or(Expiry::Never),
    })
}

pub fn run(args: &[String]) -> ExitCode {
    let args = match parse(args) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    // A script outside Terminescu keeps running; it just has no tab to label.
    let Some(pane) = pane_id() else {
        return warn("not in a Terminescu pane, status not set");
    };
    let message = CoreMessage::new(
        pane,
        CoreRequest::Status {
            label: args.label,
            color: args.color,
            expiry: args.expiry,
        },
    );
    match request(&core_socket(), &message, TIMEOUT) {
        Ok(CoreReply::Ok) => ExitCode::SUCCESS,
        Ok(CoreReply::Error { code, message }) if code == "not_built" || code == "starting" => {
            warn(&format!("status not set: {message}"))
        }
        Ok(CoreReply::Error { code, message }) => fail(&format!("{code}: {message}")),
        Err(SendError::NotRunning) => warn("Terminescu is not running, status not set"),
        Err(e) => warn(&format!("status not set: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Args, String> {
        parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn parses_status_args() {
        assert_eq!(
            p(&["Deploying", "--color", "cyan", "--until-exit"]).unwrap(),
            Args {
                label: Some("Deploying".into()),
                color: Some("cyan".into()),
                expiry: Expiry::CommandEnd
            }
        );
        assert_eq!(
            p(&["--for", "20m", "Migrating"]).unwrap().expiry,
            Expiry::After { ms: 1_200_000 }
        );
        assert_eq!(
            p(&["--clear"]).unwrap(),
            Args {
                label: None,
                color: None,
                expiry: Expiry::Never
            }
        );
        for bad in [
            &[][..],
            &["--clear", "x"],
            &["a", "b"],
            &["a", "--for", "1s", "--until-exit"],
            &["a", "--for", "soon"],
            &["a", "--bold"],
            &[" "],
        ] {
            assert!(p(bad).is_err(), "{bad:?}");
        }
    }
}

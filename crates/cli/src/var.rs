//! `terminescu var`: the side tab's variables (D21, D35, D36). Session and secret variables live
//! in the daemon, so `get`, `env` and session `set` work while the app is closed. Saved variables
//! go through the app.

use crate::common::{core_socket, fail, pane_id, parse_duration, ptyd_socket, usage_error, warn};
use std::io::{self, Read};
use std::process::ExitCode;
use std::time::Duration;
use terminescu_protocol::client::{Client, ClientError};
use terminescu_protocol::core::{CoreMessage, CoreReply, CoreRequest, SendError, request};
use terminescu_protocol::{Control, Frame, PaneId, Var, VarMode, is_var_name};

const TIMEOUT: Duration = Duration::from_secs(2);
/// `var env` runs before every prompt; a wedged daemon must not hang the shell.
const ENV_TIMEOUT: Duration = Duration::from_millis(500);
const REQ: u64 = 1;

pub fn run(args: &[String]) -> ExitCode {
    let (sub, rest) = match args.split_first() {
        Some((sub, rest)) => (sub.as_str(), rest),
        None => return usage_error("var needs a subcommand"),
    };
    match sub {
        "env" => env(rest),
        "get" => get(rest),
        "set" => set(rest),
        "unset" => unset(rest),
        "list" => list(rest),
        other => usage_error(&format!("unknown var subcommand `{other}`")),
    }
}

/// The names `var env` exported last time, space-separated, so the next call can unset the ones
/// that went away.
const NAMES_ENV: &str = "TERMINESCU_VARS";
/// `<daemon instance>:<generation>` of the variables loaded last time. Unchanged means nothing
/// to print; the instance part keeps a restarted daemon's generations from matching old ones.
const GEN_ENV: &str = "TERMINESCU_VARS_GEN";

/// Prints shell lines that bring the shell's variables in line with the pane's side tab, for
/// `eval "$(terminescu var env)"` before each prompt and command (D35). Prints nothing when
/// nothing changed, outside a pane or while the daemon is down, so the prompt never breaks.
fn env(args: &[String]) -> ExitCode {
    let fish = match args {
        [] => false,
        [f] if f == "--fish" => true,
        _ => return usage_error("var env takes only --fish"),
    };
    let Some(pane) = pane_id() else {
        return ExitCode::SUCCESS;
    };
    let Ok(mut client) = connect(ENV_TIMEOUT) else {
        return ExitCode::SUCCESS;
    };
    let Ok(Control::Vars {
        generation, vars, ..
    }) = ask(
        &mut client,
        Control::VarGet {
            req: REQ,
            pane,
            name: None,
        },
    )
    else {
        return ExitCode::SUCCESS;
    };
    let loaded = Loaded {
        key: format!("{}:{generation}", client.instance),
        names: std::env::var(NAMES_ENV).unwrap_or_default(),
    };
    if std::env::var(GEN_ENV).is_ok_and(|k| k == loaded.key) {
        return ExitCode::SUCCESS;
    }
    print!("{}", env_lines(&vars, &loaded, fish));
    ExitCode::SUCCESS
}

/// What the shell holds now, from its environment.
struct Loaded {
    key: String,
    names: String,
}

fn env_lines(vars: &[Var], loaded: &Loaded, fish: bool) -> String {
    // The daemon checks names on set; check again, since this output goes to eval.
    let vars: Vec<&Var> = vars.iter().filter(|v| is_var_name(&v.name)).collect();
    let mut out = String::new();
    for gone in loaded
        .names
        .split_whitespace()
        .filter(|n| is_var_name(n) && !vars.iter().any(|v| v.name == *n))
    {
        if fish {
            out += &format!("set -e {gone}\n");
        } else {
            out += &format!("unset {gone}\n");
        }
    }
    let names = vars
        .iter()
        .map(|v| v.name.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let mut assignments: Vec<(&str, &str)> = vars
        .iter()
        .map(|v| (v.name.as_str(), v.value.as_str()))
        .collect();
    assignments.push((NAMES_ENV, &names));
    assignments.push((GEN_ENV, &loaded.key));
    for (name, value) in assignments {
        if fish {
            out += &format!("set -gx {name} {}\n", fish_quote(value));
        } else {
            out += &format!("export {name}={}\n", sh_quote(value));
        }
    }
    out
}

/// POSIX single quotes: nothing inside is special except the quote itself.
fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// fish single quotes treat `\\` and `\'` as escapes.
fn fish_quote(value: &str) -> String {
    format!("'{}'", value.replace('\\', r"\\").replace('\'', r"\'"))
}

fn get(args: &[String]) -> ExitCode {
    let mut name = None;
    let mut reveal = false;
    for arg in args {
        match arg.as_str() {
            "--reveal" => reveal = true,
            flag if flag.starts_with("--") => {
                return usage_error(&format!("unknown option {flag}"));
            }
            n if name.is_none() => name = Some(n.to_string()),
            _ => return usage_error("var get takes one name"),
        }
    }
    let Some(name) = name else {
        return usage_error("var get needs a name");
    };
    let pane = match need_pane() {
        Ok(p) => p,
        Err(code) => return code,
    };
    let reply = call(TIMEOUT, |req| Control::VarGet {
        req,
        pane,
        name: Some(name.clone()),
    });
    match reply {
        Ok(Control::Vars { vars, .. }) => match vars.into_iter().find(|v| v.name == name) {
            // A secret printed by accident lands in scrollback and transcripts (D35).
            Some(v) if v.mode == VarMode::Secret && !reveal => fail(&format!(
                "{name} is a secret; pass --reveal to print its value"
            )),
            Some(v) => {
                println!("{}", v.value);
                ExitCode::SUCCESS
            }
            None => fail(&format!("{name} is not set")),
        },
        Ok(other) => fail(&format!("unexpected answer: {other:?}")),
        Err(e) => fail(&e),
    }
}

fn list(args: &[String]) -> ExitCode {
    if !args.is_empty() {
        return usage_error("var list takes no arguments");
    }
    let pane = match need_pane() {
        Ok(p) => p,
        Err(code) => return code,
    };
    match call(TIMEOUT, |req| Control::VarGet {
        req,
        pane,
        name: None,
    }) {
        Ok(Control::Vars { vars, .. }) => {
            for v in vars {
                let (mode, value) = match v.mode {
                    VarMode::Session => ("session", v.value),
                    VarMode::Secret => ("secret", "(hidden)".to_string()),
                };
                println!("{}\t{mode}\t{value}", v.name);
            }
            ExitCode::SUCCESS
        }
        Ok(other) => fail(&format!("unexpected answer: {other:?}")),
        Err(e) => fail(&e),
    }
}

#[derive(Debug, PartialEq)]
struct SetArgs {
    name: String,
    /// `None` reads the value from stdin, which keeps a secret out of the shell history.
    value: Option<String>,
    /// `None` is a saved variable, kept by the app.
    mode: Option<VarMode>,
    ttl: Option<Duration>,
}

fn parse_set(args: &[String]) -> Result<SetArgs, String> {
    let mut positional = Vec::new();
    let mut mode = None;
    let mut ttl = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--session" | "--secret" => {
                let m = if arg == "--secret" {
                    VarMode::Secret
                } else {
                    VarMode::Session
                };
                if mode.replace(m).is_some() {
                    return Err("pick one of --session and --secret".into());
                }
            }
            "--ttl" => {
                let text = it.next().ok_or("--ttl needs a duration")?;
                ttl = Some(parse_duration(text).ok_or(format!("`{text}` is not a duration"))?);
            }
            "-" => positional.push(arg.clone()),
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            _ => positional.push(arg.clone()),
        }
    }
    let (name, value) = match positional.as_slice() {
        [name] => (name.clone(), None),
        [name, v] if v == "-" => (name.clone(), None),
        [name, v] => (name.clone(), Some(v.clone())),
        [] => return Err("var set needs a name".into()),
        _ => return Err("quote a value that has spaces".into()),
    };
    if !is_var_name(&name) {
        return Err(format!(
            "`{name}` is not a variable name: use letters, digits and _, not starting with a digit"
        ));
    }
    if ttl.is_some() && mode.is_none() {
        return Err("--ttl needs --session or --secret".into());
    }
    Ok(SetArgs {
        name,
        value,
        mode,
        ttl,
    })
}

fn set(args: &[String]) -> ExitCode {
    let args = match parse_set(args) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let pane = match need_pane() {
        Ok(p) => p,
        Err(code) => return code,
    };
    let value = match args.value {
        Some(v) => v,
        None => match read_stdin_value() {
            Ok(v) => v,
            Err(e) => return fail(&format!("could not read the value from stdin: {e}")),
        },
    };
    let Some(mode) = args.mode else {
        return set_saved(pane, args.name, value);
    };
    let reply = call(TIMEOUT, |req| Control::VarSet {
        req,
        pane,
        name: args.name.clone(),
        value,
        mode,
        ttl_secs: args.ttl.map(|d| d.as_secs()),
    });
    done(reply)
}

/// Saved variables live in the app's database (D21). With the app closed or still starting the
/// call warns and still succeeds (D36).
fn set_saved(pane: PaneId, name: String, value: String) -> ExitCode {
    let message = CoreMessage::new(pane, CoreRequest::VarSet { name, value });
    match request(&core_socket(), &message, TIMEOUT) {
        Ok(CoreReply::Ok) => ExitCode::SUCCESS,
        Ok(CoreReply::Error { code, message }) if code == "starting" => warn(&format!(
            "not saved: {message}; --session keeps it until the Mac restarts"
        )),
        Ok(CoreReply::Error { code, message }) => fail(&format!("{code}: {message}")),
        Err(SendError::NotRunning) => {
            warn("Terminescu is not running, not saved; --session keeps it in the daemon instead")
        }
        Err(e) => warn(&format!("not saved: {e}")),
    }
}

fn unset(args: &[String]) -> ExitCode {
    let [name] = args else {
        return usage_error("var unset takes one name");
    };
    let pane = match need_pane() {
        Ok(p) => p,
        Err(code) => return code,
    };
    done(call(TIMEOUT, |req| Control::VarUnset {
        req,
        pane,
        name: name.clone(),
    }))
}

fn done(reply: Result<Control, String>) -> ExitCode {
    match reply {
        Ok(Control::Done { .. }) => ExitCode::SUCCESS,
        Ok(other) => fail(&format!("unexpected answer: {other:?}")),
        Err(e) => fail(&e),
    }
}

fn need_pane() -> Result<PaneId, ExitCode> {
    pane_id().ok_or_else(|| fail("not in a Terminescu pane (TERMINESCU_PANE_ID is unset)"))
}

/// One value from stdin, without the newline `echo` or a heredoc adds.
fn read_stdin_value() -> io::Result<String> {
    let mut value = String::new();
    io::stdin().read_to_string(&mut value)?;
    if value.ends_with('\n') {
        value.pop();
        if value.ends_with('\r') {
            value.pop();
        }
    }
    Ok(value)
}

/// Sends one request to the daemon and waits for its answer. A daemon error comes back as
/// `Err` with a message for the person.
fn call(timeout: Duration, build: impl FnOnce(u64) -> Control) -> Result<Control, String> {
    let mut client = connect(timeout)?;
    ask(&mut client, build(REQ))
}

fn connect(timeout: Duration) -> Result<Client, String> {
    Client::connect_timeout(&ptyd_socket(), "terminescu-cli", timeout).map_err(describe)
}

fn ask(client: &mut Client, request: Control) -> Result<Control, String> {
    client.send(&Frame::Control(request)).map_err(describe)?;
    let answer = client
        .recv_until(|f| match f {
            Frame::Control(
                c @ (Control::Vars { req: REQ, .. }
                | Control::Done { req: REQ }
                | Control::Error { .. }),
            ) => Some(c.clone()),
            _ => None,
        })
        .map_err(describe)?;
    match answer {
        Control::Error { code, message, .. } => Err(match code.as_str() {
            "no_side_tab" => "this pane has no side tab yet; try again in a moment".into(),
            "no_pane" => "the daemon does not know this pane".into(),
            _ => format!("{code}: {message}"),
        }),
        ok => Ok(ok),
    }
}

fn describe(e: ClientError) -> String {
    match e {
        ClientError::Io(e)
            if matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            "the Terminescu daemon is not running".into()
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(name: &str, value: &str, mode: VarMode) -> Var {
        Var {
            name: name.into(),
            value: value.into(),
            mode,
            expires_in_ms: None,
        }
    }

    #[test]
    fn env_lines_quote_values_and_unset_what_went() {
        let vars = [
            v("TICKET", "PROJ-482", VarMode::Session),
            v("NOTE", r"it's $HOME \n", VarMode::Secret),
        ];
        let loaded = Loaded {
            key: "i-1:7".into(),
            names: "OLD TICKET bad-name".into(),
        };
        assert_eq!(
            env_lines(&vars, &loaded, false),
            "unset OLD\n\
             export TICKET='PROJ-482'\n\
             export NOTE='it'\\''s $HOME \\n'\n\
             export TERMINESCU_VARS='TICKET NOTE'\n\
             export TERMINESCU_VARS_GEN='i-1:7'\n"
        );
        assert_eq!(
            env_lines(&vars[..1], &loaded, true),
            "set -e OLD\n\
             set -gx TICKET 'PROJ-482'\n\
             set -gx TERMINESCU_VARS 'TICKET'\n\
             set -gx TERMINESCU_VARS_GEN 'i-1:7'\n"
        );
    }

    fn s(args: &[&str]) -> Result<SetArgs, String> {
        parse_set(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn parses_set_args() {
        assert_eq!(
            s(&["API_TOKEN", "--secret", "--ttl", "1h"]).unwrap(),
            SetArgs {
                name: "API_TOKEN".into(),
                value: None,
                mode: Some(VarMode::Secret),
                ttl: Some(Duration::from_secs(3600)),
            }
        );
        assert_eq!(
            s(&["from_date", "2026-11-01", "--session"]).unwrap().value,
            Some("2026-11-01".into())
        );
        assert_eq!(s(&["X", "-"]).unwrap().value, None);
        assert_eq!(s(&["X", "1"]).unwrap().mode, None);
        for bad in [
            &[][..],
            &["1X", "a"],
            &["X", "a", "b"],
            &["X", "a", "--ttl", "1h"],
            &["X", "--session", "--secret"],
        ] {
            assert!(s(bad).is_err(), "{bad:?}");
        }
    }
}

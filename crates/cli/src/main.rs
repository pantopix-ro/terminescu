//! `terminescu`: the Claude Code hook and the status, var and edit tool (D34, D35, D36).
//! Arguments are parsed by hand: the hook runs in every Claude session and must start fast.

mod common;
mod edit;
mod hook;
mod status;
mod var;

use std::process::ExitCode;

const USAGE: &str = "\
usage: terminescu hook [--baseline-session <ptr> --baseline-path <ptr>... [--baseline-cwd <ptr>]]
       terminescu edit <file> [--wait]
       terminescu status <label> [--color <color>] [--for <duration> | --until-exit]
       terminescu status --clear
       terminescu var get <name> [--reveal]
       terminescu var set <name> [<value> | -] [--session | --secret] [--ttl <duration>]
       terminescu var unset <name>
       terminescu var list
       terminescu var env [--fish]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or_default();
    match args.first().map(String::as_str) {
        // Whatever happens the hook exits 0, prints nothing to stdout and returns within
        // 100 ms (D34).
        Some("hook") => hook::run(rest),
        Some("status") => status::run(rest),
        Some("var") => var::run(rest),
        Some("edit") => edit::run(rest),
        Some("--version") => {
            println!("terminescu {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help" | "-h" | "help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        _ => common::usage(),
    }
}

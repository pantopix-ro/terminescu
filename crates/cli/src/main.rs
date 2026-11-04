//! `terminescu`: the Claude Code hook and the status, var and edit tool (D34, D35).
//! Tasks 2.3 and 4.1 fill in the subcommands.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        // The hook runs in every Claude session on the Mac. Whatever happens it exits 0,
        // prints nothing to stdout and returns at once (D34).
        Some("hook") => ExitCode::SUCCESS,
        Some("--version") => {
            println!("terminescu {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(cmd) => {
            eprintln!("terminescu: `{cmd}` is not built yet");
            ExitCode::from(2)
        }
        None => {
            eprintln!("usage: terminescu <hook|status|var|edit> [args]");
            ExitCode::from(2)
        }
    }
}

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use terminescu_protocol::paths;
use terminescu_ptyd::server::{run, Options, DEFAULT_IDLE};

const USAGE: &str = "usage: terminescu-ptyd [--socket PATH] [--idle-exit-secs N] | --version";

fn main() -> ExitCode {
    let mut socket: Option<PathBuf> = None;
    let mut idle = DEFAULT_IDLE;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => {
                println!("terminescu-ptyd {} protocol {}", env!("CARGO_PKG_VERSION"), terminescu_protocol::PROTOCOL_VERSION);
                return ExitCode::SUCCESS;
            }
            "--socket" => match args.next() {
                Some(p) => socket = Some(p.into()),
                None => return usage(),
            },
            "--idle-exit-secs" => match args.next().and_then(|s| s.parse().ok()) {
                Some(n) => idle = Duration::from_secs(n),
                None => return usage(),
            },
            _ => return usage(),
        }
    }

    let opts = match socket {
        Some(socket) => {
            let dir = socket.parent().map(PathBuf::from).unwrap_or_default();
            Options { lock: socket.with_extension("lock"), core_socket: dir.join("core.sock"), socket, idle }
        }
        None => {
            let p = paths::run_paths();
            if let Err(e) = paths::ensure_private_dir(&p.dir) {
                eprintln!("terminescu-ptyd: {}: {e}", p.dir.display());
                return ExitCode::FAILURE;
            }
            Options { socket: p.ptyd_socket, lock: p.ptyd_lock, core_socket: p.core_socket, idle }
        }
    };
    match run(opts) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => {
            eprintln!("terminescu-ptyd: another daemon already serves this socket");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("terminescu-ptyd: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

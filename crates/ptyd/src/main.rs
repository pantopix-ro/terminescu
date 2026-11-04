//! `terminescu-ptyd`: owns every pane's shell (D7). Task 2.1 adds the socket server, the
//! ptys and the headless screens; this binary exists so the app bundles it as a sidecar.

use std::process::ExitCode;
use terminescu_protocol::PROTOCOL_VERSION;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("--version") => {
            println!("terminescu-ptyd {} protocol {PROTOCOL_VERSION}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("terminescu-ptyd: the daemon is not built yet (task 2.1)");
            ExitCode::from(2)
        }
    }
}

//! `terminescu-ptyd`: owns every shell, keeps a screen model per pane and serves panes to the
//! app over a unix socket (D31, D32).

pub mod screen;
pub mod server;
pub mod snapshot;
pub mod vars;

pub(crate) fn paths_uid() -> u32 {
    terminescu_protocol::paths::uid()
}

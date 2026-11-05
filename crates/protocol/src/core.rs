//! Messages on the core socket, `core.sock` (D34, D36).
//!
//! Each `terminescu` call connects, sends one [`CoreMessage`] as a control frame and reads one
//! [`CoreReply`]. Hook messages expect no reply: the hook writes and leaves.

use crate::{DecodeError, PROTOCOL_VERSION, PaneId, decode_message, encode_message};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreMessage {
    pub version: u32,
    /// The calling pane, from `TERMINESCU_PANE_ID`.
    pub pane: PaneId,
    pub request: CoreRequest,
}

impl CoreMessage {
    pub fn new(pane: PaneId, request: CoreRequest) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            pane,
            request,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CoreRequest {
    /// A Claude Code hook event, as Claude wrote it to the hook's stdin.
    Hook { event: serde_json::Value },
    /// Sets the pane's tab status, or clears it when `label` is absent (D15).
    Status {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<String>,
        expiry: Expiry,
    },
    /// Sets a saved variable on the pane's side tab.
    VarSet { name: String, value: String },
    /// Opens a file in a text pane of the caller's side tab (D27). With `wait` the pane splits
    /// beside the caller and the reply comes when the pane returns or closes.
    Edit {
        /// Absolute: the CLI resolves it against its own cwd.
        path: String,
        #[serde(default)]
        wait: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Expiry {
    Never,
    After {
        ms: u64,
    },
    /// When the command running in the pane ends, as the shell integration reports it.
    CommandEnd,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CoreReply {
    Ok,
    Error { code: String, message: String },
}

impl CoreReply {
    pub fn error(code: &str, message: impl Into<String>) -> Self {
        Self::Error {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SendError {
    /// Nothing listens: the app is not running.
    #[error("the app is not running")]
    NotRunning,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Decode(#[from] DecodeError),
    #[error("the app closed the connection")]
    Closed,
}

/// Sends one message and waits for its reply, all within `budget`.
pub fn request(
    socket: &Path,
    message: &CoreMessage,
    budget: Duration,
) -> Result<CoreReply, SendError> {
    let deadline = Instant::now() + budget;
    let mut stream = connect(socket)?;
    write_within(&mut stream, &encode_message(message), deadline)?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if let Some((reply, _)) = decode_message::<CoreReply>(&buf)? {
            return Ok(reply);
        }
        stream.set_read_timeout(Some(left(deadline)?))?;
        match stream.read(&mut chunk) {
            Ok(0) => return Err(SendError::Closed),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) => return Err(e.into()),
        }
    }
}

/// Sends one message without waiting for a reply.
pub fn post(socket: &Path, message: &CoreMessage, budget: Duration) -> Result<(), SendError> {
    let deadline = Instant::now() + budget;
    let mut stream = connect(socket)?;
    write_within(&mut stream, &encode_message(message), deadline)?;
    // Stay connected until the app hangs up: a peer that is already gone fails the app's
    // same-user check, and the message would be dropped.
    stream.shutdown(std::net::Shutdown::Write)?;
    stream.set_read_timeout(Some(left(deadline)?))?;
    let mut sink = [0u8; 64];
    while stream.read(&mut sink)? > 0 {}
    Ok(())
}

fn connect(socket: &Path) -> Result<UnixStream, SendError> {
    UnixStream::connect(socket).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused => SendError::NotRunning,
        _ => SendError::Io(e),
    })
}

fn left(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))
}

fn write_within(stream: &mut UnixStream, bytes: &[u8], deadline: Instant) -> io::Result<()> {
    stream.set_write_timeout(Some(left(deadline)?))?;
    stream.write_all(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_roundtrip() {
        let m = CoreMessage::new(
            4,
            CoreRequest::Status {
                label: Some("Deploying".into()),
                color: Some("cyan".into()),
                expiry: Expiry::After { ms: 1000 },
            },
        );
        let bytes = encode_message(&m);
        let (back, used) = decode_message::<CoreMessage>(&bytes).unwrap().unwrap();
        assert_eq!(back, m);
        assert_eq!(used, bytes.len());
        let json: serde_json::Value = serde_json::from_slice(&bytes[5..]).unwrap();
        assert_eq!(json["request"]["op"], "status");
        assert_eq!(json["request"]["expiry"]["kind"], "after");
    }

    #[test]
    fn no_listener_means_not_running() {
        let path = std::env::temp_dir().join(format!("tcore-none-{}.sock", std::process::id()));
        let m = CoreMessage::new(
            1,
            CoreRequest::Hook {
                event: serde_json::json!({}),
            },
        );
        assert!(matches!(
            request(&path, &m, Duration::from_millis(100)),
            Err(SendError::NotRunning)
        ));
    }
}

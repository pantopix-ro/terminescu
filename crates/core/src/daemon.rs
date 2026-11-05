//! The core's connection to `terminescu-ptyd` (D32).
//!
//! One reader thread per connection. Answers to requests go back to the waiting caller by `req`
//! number; output, attach and exit messages go to an event channel.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use terminescu_protocol::client::{Client, ClientError};
use terminescu_protocol::{
    Control, FEATURE_SHELL, FEATURE_VAR_MOVE, FEATURE_VARS, Frame, PaneId, PaneInfo, ShellState,
    SpawnSpec, Var, VarMode, decode, encode,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonEvent {
    Output {
        pane: PaneId,
        data: Vec<u8>,
    },
    Attached {
        pane: PaneId,
        cols: u16,
        rows: u16,
    },
    Exited {
        pane: PaneId,
        code: Option<i32>,
    },
    /// A command started or ended, or the cwd moved (statuses, D15).
    Shell {
        pane: PaneId,
        shell: ShellState,
    },
    /// A side tab's session or secret variables changed (D21).
    VarsChanged {
        side_tab: String,
    },
    /// The connection ended; the client is no longer usable.
    Disconnected,
}

#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("daemon did not answer in time")]
    Timeout,
    #[error("daemon error {code}: {message}")]
    Daemon { code: String, message: String },
    #[error("unexpected answer: {0:?}")]
    Unexpected(Box<Control>),
}

type Pending = Arc<Mutex<HashMap<u64, Sender<Control>>>>;

pub struct DaemonClient {
    writer: Mutex<UnixStream>,
    pending: Pending,
    next_req: AtomicU64,
    pub version: u32,
    pub instance: String,
    /// The daemon pushes shell changes (`FEATURE_SHELL`); an older one leaves them to polling.
    pub pushes_shell: bool,
    /// The daemon serves `SideVar*` requests and `VarsChanged` pushes (`FEATURE_VARS`).
    pub side_vars: bool,
    /// The daemon moves variables between side tabs (`FEATURE_VAR_MOVE`).
    pub var_move: bool,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl DaemonClient {
    /// Connects, completes the handshake and starts the reader thread, which sends every
    /// event to `events` and ends with `Disconnected`.
    pub fn connect(socket: &Path, events: Sender<DaemonEvent>) -> Result<Self, DaemonError> {
        let client = Client::connect_with(
            socket,
            concat!("terminescu-core ", env!("CARGO_PKG_VERSION")),
            &[FEATURE_SHELL, FEATURE_VARS, FEATURE_VAR_MOVE],
        )?;
        let version = client.version;
        let instance = client.instance.clone();
        let pushes_shell = client.features.iter().any(|f| f == FEATURE_SHELL);
        let side_vars = client.features.iter().any(|f| f == FEATURE_VARS);
        let var_move = client.features.iter().any(|f| f == FEATURE_VAR_MOVE);
        let (stream, leftover) = client.into_parts();
        stream.set_read_timeout(None)?;
        let reader = stream.try_clone()?;
        let pending: Pending = Arc::default();
        let p = pending.clone();
        std::thread::Builder::new()
            .name("daemon-read".into())
            .spawn(move || read_loop(reader, leftover, p, events))?;
        Ok(Self {
            writer: Mutex::new(stream),
            pending,
            next_req: AtomicU64::new(1),
            version,
            instance,
            pushes_shell,
            side_vars,
            var_move,
        })
    }

    fn send(&self, frame: &Frame) -> io::Result<()> {
        lock(&self.writer).write_all(&encode(frame))
    }

    fn request(&self, make: impl FnOnce(u64) -> Control) -> Result<Control, DaemonError> {
        let req = self.next_req.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        lock(&self.pending).insert(req, tx);
        if let Err(e) = self.send(&Frame::Control(make(req))) {
            lock(&self.pending).remove(&req);
            return Err(e.into());
        }
        let answer = rx.recv_timeout(REQUEST_TIMEOUT);
        lock(&self.pending).remove(&req);
        match answer {
            Ok(Control::Error { code, message, .. }) => Err(DaemonError::Daemon { code, message }),
            Ok(c) => Ok(c),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(DaemonError::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ClientError::Closed.into()),
        }
    }

    pub fn spawn(&self, spec: SpawnSpec) -> Result<PaneId, DaemonError> {
        match self.request(|req| Control::Spawn { req, spec })? {
            Control::Spawned { pane, .. } => Ok(pane),
            other => Err(DaemonError::Unexpected(Box::new(other))),
        }
    }

    pub fn list(&self) -> Result<Vec<PaneInfo>, DaemonError> {
        match self.request(|req| Control::List { req })? {
            Control::Panes { panes, .. } => Ok(panes),
            other => Err(DaemonError::Unexpected(Box::new(other))),
        }
    }

    pub fn attach(&self, pane: PaneId) -> io::Result<()> {
        self.send(&Frame::Control(Control::Attach { pane }))
    }

    pub fn detach(&self, pane: PaneId) -> io::Result<()> {
        self.send(&Frame::Control(Control::Detach { pane }))
    }

    pub fn resize(&self, pane: PaneId, cols: u16, rows: u16) -> io::Result<()> {
        self.send(&Frame::Control(Control::Resize { pane, cols, rows }))
    }

    pub fn input(&self, pane: PaneId, data: Vec<u8>) -> io::Result<()> {
        self.send(&Frame::Input { pane, data })
    }

    pub fn close(&self, pane: PaneId) -> io::Result<()> {
        self.send(&Frame::Control(Control::Close { pane }))
    }

    pub fn set_side_tab(&self, pane: PaneId, side_tab: &str) -> io::Result<()> {
        self.send(&Frame::Control(Control::SetSideTab {
            pane,
            side_tab: side_tab.into(),
        }))
    }

    fn need_side_vars(&self) -> Result<(), DaemonError> {
        if self.side_vars {
            Ok(())
        } else {
            Err(DaemonError::Daemon {
                code: "old_daemon".into(),
                message: "the running daemon predates side tab variables; it updates once its \
                          last shell exits"
                    .into(),
            })
        }
    }

    fn done(&self, make: impl FnOnce(u64) -> Control) -> Result<(), DaemonError> {
        self.need_side_vars()?;
        match self.request(make)? {
            Control::Done { .. } => Ok(()),
            other => Err(DaemonError::Unexpected(Box::new(other))),
        }
    }

    /// Sets a session or secret variable on a side tab (D21).
    pub fn side_var_set(
        &self,
        side_tab: &str,
        name: &str,
        value: String,
        mode: VarMode,
        ttl_secs: Option<u64>,
    ) -> Result<(), DaemonError> {
        self.done(|req| Control::SideVarSet {
            req,
            side_tab: side_tab.into(),
            name: name.into(),
            value,
            mode,
            ttl_secs,
        })
    }

    pub fn side_var_unset(&self, side_tab: &str, name: &str) -> Result<(), DaemonError> {
        self.done(|req| Control::SideVarUnset {
            req,
            side_tab: side_tab.into(),
            name: name.into(),
        })
    }

    /// The side tab's session and secret variables; secret values come back empty.
    pub fn side_var_list(&self, side_tab: &str) -> Result<Vec<Var>, DaemonError> {
        self.need_side_vars()?;
        match self.request(|req| Control::SideVarList {
            req,
            side_tab: side_tab.into(),
        })? {
            Control::Vars { vars, .. } => Ok(vars),
            other => Err(DaemonError::Unexpected(Box::new(other))),
        }
    }

    /// Wipes a closed side tab's variables.
    pub fn side_vars_drop(&self, side_tab: &str) -> Result<(), DaemonError> {
        self.done(|req| Control::SideVarsDrop {
            req,
            side_tab: side_tab.into(),
        })
    }

    /// Moves variables between side tabs, renamed by `moves` (`from` name, `to` name).
    pub fn side_vars_move(
        &self,
        from: &str,
        to: &str,
        moves: &[(String, String)],
    ) -> Result<(), DaemonError> {
        if !self.var_move {
            return Err(DaemonError::Daemon {
                code: "unsupported".into(),
                message: "the terminal daemon is too old to move variables; it updates once \
                          every shell has closed"
                    .into(),
            });
        }
        self.done(|req| Control::SideVarsMove {
            req,
            from: from.into(),
            to: to.into(),
            moves: moves
                .iter()
                .map(|(a, b)| terminescu_protocol::VarMove {
                    from: a.clone(),
                    to: b.clone(),
                })
                .collect(),
        })
    }

    /// Ends the connection; the reader thread then reports `Disconnected`.
    pub fn shutdown(&self) {
        let _ = lock(&self.writer).shutdown(std::net::Shutdown::Both);
    }
}

fn read_loop(
    mut stream: UnixStream,
    mut buf: Vec<u8>,
    pending: Pending,
    events: Sender<DaemonEvent>,
) {
    let mut chunk = vec![0u8; 64 * 1024];
    'conn: loop {
        loop {
            let (frame, used) = match decode(&buf) {
                Ok(Some(x)) => x,
                Ok(None) => break,
                Err(e) => {
                    eprintln!("terminescu: bad frame from the daemon: {e}");
                    break 'conn;
                }
            };
            buf.drain(..used);
            let event = match frame {
                Frame::Output { pane, data } => Some(DaemonEvent::Output { pane, data }),
                Frame::Input { .. } => None,
                Frame::Control(c) => match c {
                    Control::Attached { pane, cols, rows } => {
                        Some(DaemonEvent::Attached { pane, cols, rows })
                    }
                    Control::Exited { pane, code } => Some(DaemonEvent::Exited { pane, code }),
                    Control::Shell { pane, shell } => Some(DaemonEvent::Shell { pane, shell }),
                    Control::VarsChanged { side_tab } => {
                        Some(DaemonEvent::VarsChanged { side_tab })
                    }
                    Control::Spawned { req, .. }
                    | Control::Panes { req, .. }
                    | Control::Vars { req, .. }
                    | Control::Done { req }
                    | Control::Error { req: Some(req), .. } => {
                        if let Some(tx) = lock(&pending).remove(&req) {
                            let _ = tx.send(c);
                        }
                        None
                    }
                    Control::Error { code, message, .. } => {
                        eprintln!("terminescu: daemon error {code}: {message}");
                        None
                    }
                    _ => None,
                },
            };
            if let Some(e) = event
                && events.send(e).is_err()
            {
                break 'conn;
            }
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    // Waiting callers see their channel close.
    lock(&pending).clear();
    let _ = events.send(DaemonEvent::Disconnected);
}

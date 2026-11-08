//! The daemon: a unix socket server that owns the panes (D32).
//!
//! Threads, no async runtime: one accept loop, a reader and a writer thread per client, and a
//! reader, a writer and a waiter thread per pane. Each client has a bounded queue; a client that
//! falls behind gets disconnected and reattaches with a fresh snapshot.

use crate::osc::{OscScanner, ShellTracker};
use crate::screen::Screen;
use crate::vars::Vars;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use terminescu_protocol::paths::{CORE_SOCKET_ENV, PANE_ID_ENV, PTYD_SOCKET_ENV, peer_uid};
use terminescu_protocol::{
    Control, FEATURE_CLEAR, FEATURE_SHELL, FEATURE_VAR_MOVE, FEATURE_VARS, Frame, PROTOCOL_VERSION,
    PaneId, PaneInfo, ShellState, SpawnSpec, decode, encode, is_var_name, negotiate,
};

/// Frames a client may have queued before it counts as stuck.
const CLIENT_QUEUE: usize = 1024;
const READ_CHUNK: usize = 64 * 1024;
const SNAPSHOT_PIECE: usize = 1024 * 1024;
pub const DEFAULT_IDLE: Duration = Duration::from_secs(600);

pub struct Options {
    pub socket: PathBuf,
    pub lock: PathBuf,
    pub core_socket: PathBuf,
    /// The `terminescu` CLI the shell integration calls for `var env` and puts on `PATH`
    /// (D36). Without it the integration still reports prompts, exit codes and the cwd.
    pub cli: Option<PathBuf>,
    /// Exit after this long with no live pane and no client (D32).
    pub idle: Duration,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // A thread that panicked while holding a lock leaves plain data behind; keep serving.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

struct Subscriber {
    client: u64,
    tx: SyncSender<Vec<u8>>,
}

struct PaneState {
    screen: Screen,
    subscribers: Vec<Subscriber>,
    side_tab: Option<String>,
    exited: Option<i32>,
    osc: OscScanner,
    shell: ShellTracker,
}

struct Pane {
    id: PaneId,
    pid: Option<u32>,
    state: Mutex<PaneState>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    input: Sender<Vec<u8>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

struct ClientHandle {
    tx: SyncSender<Vec<u8>>,
    stream: UnixStream,
    /// Asked for `Control::Shell` pushes in its `Hello`.
    shell: bool,
    /// Asked for `Control::VarsChanged` pushes.
    vars: bool,
}

struct Shared {
    opts: Options,
    panes: Mutex<HashMap<PaneId, Arc<Pane>>>,
    clients: Mutex<HashMap<u64, ClientHandle>>,
    next_pane: AtomicU32,
    next_client: AtomicU64,
    last_activity: Mutex<Instant>,
    instance: String,
    vars: Mutex<Vars>,
}

impl Shared {
    fn touch(&self) {
        *lock(&self.last_activity) = Instant::now();
    }

    /// Queues `bytes` for a client; a full queue disconnects the client.
    fn push(&self, client: u64, tx: &SyncSender<Vec<u8>>, bytes: Vec<u8>) -> bool {
        match tx.try_send(bytes) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                eprintln!("ptyd: client {client} fell behind, disconnecting it");
                self.drop_client(client);
                false
            }
            Err(TrySendError::Disconnected(_)) => false,
        }
    }

    fn drop_client(&self, client: u64) {
        if let Some(c) = lock(&self.clients).remove(&client) {
            let _ = c.stream.shutdown(std::net::Shutdown::Both);
        }
        self.touch();
    }

    fn broadcast(&self, frame: &Frame) {
        let bytes = encode(frame);
        let targets: Vec<(u64, SyncSender<Vec<u8>>)> = lock(&self.clients)
            .iter()
            .map(|(id, c)| (*id, c.tx.clone()))
            .collect();
        for (id, tx) in targets {
            self.push(id, &tx, bytes.clone());
        }
    }

    /// A pane's new shell state, to the clients that asked for it.
    fn broadcast_shell(&self, pane: PaneId, shell: ShellState) {
        let bytes = encode(&Frame::Control(Control::Shell { pane, shell }));
        let targets: Vec<(u64, SyncSender<Vec<u8>>)> = lock(&self.clients)
            .iter()
            .filter(|(_, c)| c.shell)
            .map(|(id, c)| (*id, c.tx.clone()))
            .collect();
        for (id, tx) in targets {
            self.push(id, &tx, bytes.clone());
        }
    }

    /// Tells the clients that asked that a side tab's variables changed. Names no value.
    fn broadcast_vars(&self, side_tab: &str) {
        let bytes = encode(&Frame::Control(Control::VarsChanged {
            side_tab: side_tab.into(),
        }));
        let targets: Vec<(u64, SyncSender<Vec<u8>>)> = lock(&self.clients)
            .iter()
            .filter(|(_, c)| c.vars)
            .map(|(id, c)| (*id, c.tx.clone()))
            .collect();
        for (id, tx) in targets {
            self.push(id, &tx, bytes.clone());
        }
    }

    fn pane(&self, id: PaneId) -> Option<Arc<Pane>> {
        lock(&self.panes).get(&id).cloned()
    }

    /// The side tab whose variables a pane sees.
    fn side_tab_of(&self, pane: PaneId, req: u64) -> Result<String, Control> {
        let p = self
            .pane(pane)
            .ok_or_else(|| error("no_pane", format!("pane {pane}"), Some(req)))?;
        let side = lock(&p.state).side_tab.clone();
        side.ok_or_else(|| {
            error(
                "no_side_tab",
                format!("pane {pane} has no side tab yet"),
                Some(req),
            )
        })
    }

    fn spawn(self: &Arc<Self>, spec: SpawnSpec) -> io::Result<PaneId> {
        let id = self.next_pane.fetch_add(1, Ordering::Relaxed);
        let (cols, rows) = crate::screen::clamp_size(spec.cols, spec.rows);
        let scrollback = spec.scrollback.map_or(crate::screen::SCROLLBACK, |n| {
            (n as usize).clamp(
                *crate::screen::SCROLLBACK_RANGE.start(),
                *crate::screen::SCROLLBACK_RANGE.end(),
            )
        });
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(io::Error::other)?;
        let login_shell = spec.argv.as_ref().is_none_or(|a| a.is_empty());
        let mut cmd = match &spec.argv {
            Some(argv) if !argv.is_empty() => {
                let mut c = CommandBuilder::new(&argv[0]);
                c.args(&argv[1..]);
                c
            }
            // The login shell (D8): portable-pty runs `$SHELL` with a leading dash in argv[0];
            // `shell::inject` swaps in the integration for zsh, bash and fish.
            _ => CommandBuilder::new_default_prog(),
        };
        let cwd = spec.cwd.clone().or_else(|| std::env::var("HOME").ok());
        if let Some(cwd) = cwd {
            cmd.cwd(cwd);
        }
        for (key, _) in std::env::vars_os() {
            let k = key.to_string_lossy();
            // A daemon started from a pane must not hand that pane's bookkeeping on.
            if k.starts_with("CLAUDE") || (k.starts_with("TERMINESCU_") && k != "TERMINESCU_HOME") {
                cmd.env_remove(key);
            }
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "Terminescu");
        cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        if std::env::var_os("LANG").is_none() {
            cmd.env("LANG", "en_US.UTF-8");
        }
        // BSD `ls` and friends print colour only with CLICOLOR. A value the person set, or
        // NO_COLOR (no-color.org), wins.
        if std::env::var_os("CLICOLOR").is_none() && std::env::var_os("NO_COLOR").is_none() {
            cmd.env("CLICOLOR", "1");
        }
        cmd.env(PANE_ID_ENV, id.to_string());
        cmd.env(PTYD_SOCKET_ENV, &self.opts.socket);
        cmd.env(CORE_SOCKET_ENV, &self.opts.core_socket);
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }
        if login_shell {
            crate::shell::inject(&mut cmd, &self.shell_dir(), self.opts.cli.as_ref());
        }

        let mut child = pair.slave.spawn_command(cmd).map_err(io::Error::other)?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().map_err(io::Error::other)?;
        let mut writer = pair.master.take_writer().map_err(io::Error::other)?;
        let (input, input_rx) = mpsc::channel::<Vec<u8>>();
        let pane = Arc::new(Pane {
            id,
            pid: child.process_id(),
            state: Mutex::new(PaneState {
                screen: Screen::new(cols, rows, scrollback),
                subscribers: Vec::new(),
                side_tab: spec.side_tab,
                exited: None,
                osc: OscScanner::default(),
                shell: ShellTracker::default(),
            }),
            master: Mutex::new(pair.master),
            input,
            killer: Mutex::new(child.clone_killer()),
        });
        lock(&self.panes).insert(id, pane.clone());

        // Output: feed the screen model and fan out, under the pane lock so an attach sees
        // either all of a chunk in its snapshot or all of it as live output.
        let shared = self.clone();
        let p = pane.clone();
        std::thread::Builder::new()
            .name(format!("pane-{id}-read"))
            .spawn(move || {
                let mut buf = vec![0u8; READ_CHUNK];
                loop {
                    let n = match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    let frame = encode(&Frame::Output {
                        pane: p.id,
                        data: buf[..n].to_vec(),
                    });
                    let mut st = lock(&p.state);
                    let changed = feed_guarded(&mut st, &buf[..n], scrollback, p.id);
                    st.subscribers
                        .retain(|s| shared.push(s.client, &s.tx, frame.clone()));
                    // One push per chunk: the counters carry runs that started and ended in it.
                    let pushed = changed.then(|| st.shell.state.clone());
                    drop(st);
                    if let Some(state) = pushed {
                        shared.broadcast_shell(p.id, state);
                    }
                }
            })?;

        std::thread::Builder::new()
            .name(format!("pane-{id}-write"))
            .spawn(move || {
                for data in input_rx {
                    if writer
                        .write_all(&data)
                        .and_then(|_| writer.flush())
                        .is_err()
                    {
                        break;
                    }
                }
            })?;

        let shared = self.clone();
        let p = pane;
        std::thread::Builder::new()
            .name(format!("pane-{id}-wait"))
            .spawn(move || {
                let code = child.wait().ok().map(|s| s.exit_code() as i32);
                lock(&p.state).exited = Some(code.unwrap_or(-1));
                shared.broadcast(&Frame::Control(Control::Exited { pane: p.id, code }));
                shared.touch();
            })?;
        Ok(id)
    }

    fn info(&self, pane: &Pane) -> PaneInfo {
        let (cols, rows, side_tab, exited, shell) = {
            let st = lock(&pane.state);
            let (c, r) = st.screen.size();
            (c, r, st.side_tab.clone(), st.exited, st.shell.state.clone())
        };
        let foreground = if exited.is_none() {
            lock(&pane.master)
                .process_group_leader()
                .and_then(process_name)
        } else {
            None
        };
        PaneInfo {
            pane: pane.id,
            pid: pane.pid,
            cols,
            rows,
            side_tab,
            foreground,
            exited,
            shell,
        }
    }

    /// Where `run` wrote the shell integration: next to the socket, in the private run folder.
    fn shell_dir(&self) -> PathBuf {
        self.opts
            .socket
            .parent()
            .unwrap_or(Path::new("."))
            .join("shell")
    }

    fn live_panes(&self) -> usize {
        lock(&self.panes)
            .values()
            .filter(|p| lock(&p.state).exited.is_none())
            .count()
    }
}

fn process_name(pid: libc::pid_t) -> Option<String> {
    let mut buf = [0u8; 256];
    // SAFETY: the buffer is valid for its full length; proc_name writes at most that many bytes.
    let n = unsafe { libc::proc_name(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    (n > 0).then(|| String::from_utf8_lossy(&buf[..n as usize]).into_owned())
}

fn no_sigpipe(stream: &UnixStream) {
    let on: libc::c_int = 1;
    // SAFETY: valid fd and a c_int option value of the right size.
    unsafe {
        libc::setsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_NOSIGPIPE,
            (&on as *const libc::c_int).cast(),
            size_of::<libc::c_int>() as libc::socklen_t,
        );
    }
}

/// Feeds a chunk to the screen model and the shell scanner. A panic there (a bug in the
/// terminal model on odd output) costs the model its history, not the pane: the model starts
/// over at the same size and the output keeps flowing to the attached terminals.
fn feed_guarded(st: &mut PaneState, bytes: &[u8], scrollback: usize, pane: PaneId) -> bool {
    feed_with(st, bytes, scrollback, pane, |screen, b| screen.feed(b))
}

fn feed_with(
    st: &mut PaneState,
    bytes: &[u8],
    scrollback: usize,
    pane: PaneId,
    feed: impl FnOnce(&mut Screen, &[u8]),
) -> bool {
    let (cols, rows) = st.screen.size();
    let fed = std::panic::catch_unwind(AssertUnwindSafe(|| {
        feed(&mut st.screen, bytes);
        let PaneState { osc, shell, .. } = &mut *st;
        let mut changed = false;
        osc.feed(bytes, |payload| changed |= shell.apply(payload));
        changed
    }));
    fed.unwrap_or_else(|_| {
        eprintln!("terminescu-ptyd: pane {pane}: the screen model failed; it starts over");
        st.screen = Screen::new(cols, rows, scrollback);
        st.osc = OscScanner::default();
        false
    })
}

fn reply(tx: &SyncSender<Vec<u8>>, c: Control) {
    let _ = tx.try_send(encode(&Frame::Control(c)));
}

fn error(code: &str, message: impl Into<String>, req: Option<u64>) -> Control {
    Control::Error {
        code: code.into(),
        message: message.into(),
        req,
    }
}

fn serve_client(shared: Arc<Shared>, mut stream: UnixStream) {
    let id = shared.next_client.fetch_add(1, Ordering::Relaxed);
    let (tx, rx): (SyncSender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::sync_channel(CLIENT_QUEUE);
    let Ok(mut out) = stream.try_clone() else {
        return;
    };
    let writer = std::thread::spawn(move || {
        for bytes in rx {
            if out.write_all(&bytes).is_err() {
                break;
            }
        }
        let _ = out.shutdown(std::net::Shutdown::Both);
    });

    let mut buf = Vec::new();
    let mut chunk = vec![0u8; READ_CHUNK];
    let mut greeted = false;
    'conn: loop {
        let n = match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        buf.extend_from_slice(&chunk[..n]);
        loop {
            let (frame, used) = match decode(&buf) {
                Ok(Some(x)) => x,
                Ok(None) => break,
                Err(e) => {
                    reply(&tx, error("bad_frame", e.to_string(), None));
                    break 'conn;
                }
            };
            buf.drain(..used);
            if !greeted {
                let Frame::Control(Control::Hello {
                    versions,
                    client,
                    features,
                }) = frame
                else {
                    reply(
                        &tx,
                        error("hello_first", "the first message must be hello", None),
                    );
                    break 'conn;
                };
                let Some(version) = negotiate(&versions, &terminescu_protocol::APP_SUPPORTED)
                else {
                    reply(
                        &tx,
                        error(
                            "version",
                            format!(
                                "{client} offers {versions:?}; this daemon speaks {PROTOCOL_VERSION}"
                            ),
                            None,
                        ),
                    );
                    break 'conn;
                };
                reply(
                    &tx,
                    Control::Welcome {
                        version,
                        daemon: format!("terminescu-ptyd {}", env!("CARGO_PKG_VERSION")),
                        instance: shared.instance.clone(),
                        features: features
                            .iter()
                            .filter(|f| {
                                [FEATURE_SHELL, FEATURE_VARS, FEATURE_VAR_MOVE, FEATURE_CLEAR]
                                    .contains(&f.as_str())
                            })
                            .cloned()
                            .collect(),
                    },
                );
                let Ok(handle) = stream.try_clone() else {
                    break 'conn;
                };
                lock(&shared.clients).insert(
                    id,
                    ClientHandle {
                        tx: tx.clone(),
                        stream: handle,
                        shell: features.iter().any(|f| f == FEATURE_SHELL),
                        vars: features.iter().any(|f| f == FEATURE_VARS),
                    },
                );
                greeted = true;
                continue;
            }
            // A panic on one message ends that message, not the connection and its panes.
            if std::panic::catch_unwind(AssertUnwindSafe(|| handle(&shared, id, &tx, frame)))
                .is_err()
            {
                eprintln!("terminescu-ptyd: client {id}: a message failed and was dropped");
            }
        }
    }
    shared.drop_client(id);
    drop(tx);
    let _ = writer.join();
}

fn handle(shared: &Arc<Shared>, client: u64, tx: &SyncSender<Vec<u8>>, frame: Frame) {
    match frame {
        Frame::Input { pane, data } => {
            if let Some(p) = shared.pane(pane) {
                let _ = p.input.send(data);
            }
        }
        Frame::Output { .. } => reply(tx, error("unexpected", "clients do not send output", None)),
        Frame::Control(c) => match c {
            Control::Spawn { req, spec } => match shared.spawn(spec) {
                Ok(pane) => reply(tx, Control::Spawned { req, pane }),
                Err(e) => reply(tx, error("spawn", e.to_string(), Some(req))),
            },
            Control::Attach { pane } => {
                let Some(p) = shared.pane(pane) else {
                    return reply(tx, error("no_pane", format!("pane {pane}"), None));
                };
                let mut st = lock(&p.state);
                let (cols, rows) = st.screen.size();
                st.subscribers.retain(|s| s.client != client);
                // The snapshot goes out in pieces well under MAX_FRAME; the terminal parser on
                // the other side does not care where a piece ends.
                let snapshot = st.screen.snapshot();
                let ok = shared.push(
                    client,
                    tx,
                    encode(&Frame::Control(Control::Attached { pane, cols, rows })),
                ) && snapshot.chunks(SNAPSHOT_PIECE).all(|piece| {
                    shared.push(
                        client,
                        tx,
                        encode(&Frame::Output {
                            pane,
                            data: piece.to_vec(),
                        }),
                    )
                });
                if ok {
                    st.subscribers.push(Subscriber {
                        client,
                        tx: tx.clone(),
                    });
                    if let Some(code) = st.exited {
                        shared.push(
                            client,
                            tx,
                            encode(&Frame::Control(Control::Exited {
                                pane,
                                code: Some(code),
                            })),
                        );
                    }
                }
            }
            Control::Detach { pane } => {
                if let Some(p) = shared.pane(pane) {
                    lock(&p.state).subscribers.retain(|s| s.client != client);
                }
            }
            Control::Resize { pane, cols, rows } => {
                if let Some(p) = shared.pane(pane) {
                    let (cols, rows) = crate::screen::clamp_size(cols, rows);
                    let mut st = lock(&p.state);
                    st.screen.resize(cols, rows);
                    let _ = lock(&p.master).resize(PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    });
                }
            }
            Control::Clear { pane, keep_screen } => {
                // Only the model: the terminals clear themselves, and the shell never hears of it.
                if let Some(p) = shared.pane(pane) {
                    lock(&p.state).screen.clear(keep_screen);
                }
            }
            Control::Close { pane } => {
                if let Some(p) = lock(&shared.panes).remove(&pane) {
                    // A reaped child's pid may belong to another process by now.
                    if lock(&p.state).exited.is_none() {
                        let _ = lock(&p.killer).kill();
                    }
                }
                shared.touch();
            }
            Control::List { req } => {
                let panes: Vec<Arc<Pane>> = lock(&shared.panes).values().cloned().collect();
                let mut infos: Vec<PaneInfo> = panes.iter().map(|p| shared.info(p)).collect();
                infos.sort_by_key(|i| i.pane);
                reply(tx, Control::Panes { req, panes: infos });
            }
            Control::SetSideTab { pane, side_tab } => {
                if let Some(p) = shared.pane(pane) {
                    lock(&p.state).side_tab = Some(side_tab);
                }
            }
            Control::VarSet {
                req,
                pane,
                name,
                value,
                mode,
                ttl_secs,
            } => {
                if !is_var_name(&name) {
                    return reply(
                        tx,
                        error(
                            "bad_name",
                            format!("`{name}` cannot be exported"),
                            Some(req),
                        ),
                    );
                }
                match shared.side_tab_of(pane, req) {
                    Ok(side) => {
                        let ttl = ttl_secs.map(Duration::from_secs);
                        lock(&shared.vars).set(&side, &name, value, mode, ttl);
                        reply(tx, Control::Done { req });
                        shared.broadcast_vars(&side);
                    }
                    Err(e) => reply(tx, e),
                }
            }
            Control::VarUnset { req, pane, name } => match shared.side_tab_of(pane, req) {
                Ok(side) => {
                    lock(&shared.vars).unset(&side, &name);
                    reply(tx, Control::Done { req });
                    shared.broadcast_vars(&side);
                }
                Err(e) => reply(tx, e),
            },
            Control::VarGet { req, pane, name } => match shared.side_tab_of(pane, req) {
                Ok(side) => {
                    let (generation, vars) = lock(&shared.vars).get(&side, name.as_deref());
                    reply(
                        tx,
                        Control::Vars {
                            req,
                            generation,
                            vars,
                        },
                    );
                }
                Err(e) => reply(tx, e),
            },
            Control::SideVarSet {
                req,
                side_tab,
                name,
                value,
                mode,
                ttl_secs,
            } => {
                if !is_var_name(&name) {
                    return reply(
                        tx,
                        error(
                            "bad_name",
                            format!("`{name}` cannot be exported"),
                            Some(req),
                        ),
                    );
                }
                let ttl = ttl_secs.map(Duration::from_secs);
                lock(&shared.vars).set(&side_tab, &name, value, mode, ttl);
                reply(tx, Control::Done { req });
                shared.broadcast_vars(&side_tab);
            }
            Control::SideVarUnset {
                req,
                side_tab,
                name,
            } => {
                lock(&shared.vars).unset(&side_tab, &name);
                reply(tx, Control::Done { req });
                shared.broadcast_vars(&side_tab);
            }
            Control::SideVarList { req, side_tab } => {
                let (generation, vars) = lock(&shared.vars).list_masked(&side_tab);
                reply(
                    tx,
                    Control::Vars {
                        req,
                        generation,
                        vars,
                    },
                );
            }
            Control::SideVarsDrop { req, side_tab } => {
                let had = lock(&shared.vars).drop_side(&side_tab);
                reply(tx, Control::Done { req });
                if had {
                    shared.broadcast_vars(&side_tab);
                }
            }
            Control::SideVarsMove {
                req,
                from,
                to,
                moves,
            } => {
                if let Some(bad) = moves.iter().find(|m| !is_var_name(&m.to)) {
                    return reply(
                        tx,
                        error(
                            "bad_name",
                            format!("`{}` cannot be exported", bad.to),
                            Some(req),
                        ),
                    );
                }
                let moves: Vec<(String, String)> =
                    moves.into_iter().map(|m| (m.from, m.to)).collect();
                let moved = lock(&shared.vars).move_vars(&from, &to, &moves);
                reply(tx, Control::Done { req });
                if moved {
                    shared.broadcast_vars(&from);
                    shared.broadcast_vars(&to);
                }
            }
            other => reply(tx, error("unexpected", format!("{other:?}"), None)),
        },
    }
}

/// Takes the lock file, so only one daemon serves a socket. `Ok(None)` means another daemon
/// holds it.
fn take_lock(path: &Path) -> io::Result<Option<File>> {
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    // SAFETY: valid fd owned by `file`.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let err = io::Error::last_os_error();
        return if err.raw_os_error() == Some(libc::EWOULDBLOCK) {
            Ok(None)
        } else {
            Err(err)
        };
    }
    Ok(Some(file))
}

/// Runs the daemon until it has been idle for `opts.idle`. Returns `Ok(false)` without serving
/// when another daemon already owns the socket.
pub fn run(opts: Options) -> io::Result<bool> {
    // Shells inherit signal dispositions, and Rust starts with SIGPIPE ignored. Restore the
    // default so `yes | head` ends the way it does in any terminal; our own sockets opt out
    // with SO_NOSIGPIPE instead.
    // SAFETY: setting a signal disposition to its default has no preconditions.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };

    let Some(_lock) = take_lock(&opts.lock)? else {
        return Ok(false);
    };
    // We hold the lock, so a socket file left here belongs to a daemon that died.
    let _ = std::fs::remove_file(&opts.socket);
    let listener = UnixListener::bind(&opts.socket)?;
    if let Some(dir) = opts.socket.parent()
        && let Err(e) = crate::shell::install(&dir.join("shell"))
    {
        eprintln!("ptyd: shell integration not installed, shells start without it: {e}");
    }
    std::fs::set_permissions(&opts.socket, std::fs::Permissions::from_mode(0o600))?;

    let shared = Arc::new(Shared {
        opts,
        panes: Mutex::new(HashMap::new()),
        clients: Mutex::new(HashMap::new()),
        next_pane: AtomicU32::new(1),
        next_client: AtomicU64::new(1),
        last_activity: Mutex::new(Instant::now()),
        vars: Mutex::new(Vars::default()),
        instance: format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ),
    });

    let idle = shared.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(200).min(idle.opts.idle));
            // Expired variables leave memory on time, whether or not a shell asks (D21).
            let expired = lock(&idle.vars).sweep();
            for side in expired {
                idle.broadcast_vars(&side);
            }
            let quiet = lock(&idle.clients).is_empty() && idle.live_panes() == 0;
            if !quiet {
                idle.touch();
            } else if lock(&idle.last_activity).elapsed() >= idle.opts.idle {
                let _ = std::fs::remove_file(&idle.opts.socket);
                std::process::exit(0);
            }
        }
    });

    let me = crate::paths_uid();
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        no_sigpipe(&stream);
        match peer_uid(&stream) {
            Ok(uid) if uid == me => {}
            _ => {
                eprintln!("ptyd: refused a connection from another user");
                continue;
            }
        }
        let shared = shared.clone();
        std::thread::spawn(move || serve_client(shared, stream));
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> PaneState {
        let mut screen = Screen::new(80, 24, 1000);
        screen.feed(b"history\r\n");
        PaneState {
            screen,
            subscribers: Vec::new(),
            side_tab: None,
            exited: None,
            osc: OscScanner::default(),
            shell: ShellTracker::default(),
        }
    }

    #[test]
    fn a_panic_in_the_screen_model_starts_it_over_at_the_same_size() {
        let mut st = state();
        st.screen.resize(120, 40);
        let changed = feed_with(&mut st, b"x", 1000, 7, |_, _| panic!("model bug"));
        assert!(!changed);
        assert_eq!(st.screen.size(), (120, 40));
        // The new model takes output again.
        assert!(!feed_guarded(&mut st, b"after\r\n", 1000, 7));
        assert!(String::from_utf8_lossy(&st.screen.snapshot()).contains("after"));
        assert!(!String::from_utf8_lossy(&st.screen.snapshot()).contains("history"));
    }

    #[test]
    fn a_chunk_still_reaches_the_shell_scanner() {
        let mut st = state();
        assert!(feed_guarded(
            &mut st,
            b"\x1b]7;file://host/tmp/x\x07",
            1000,
            7
        ));
        assert_eq!(st.shell.state.cwd.as_deref(), Some("/tmp/x"));
    }
}

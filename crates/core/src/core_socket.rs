//! The core socket, `core.sock`: where `terminescu hook`, `status` and saved `var set` arrive
//! (D34, D36).
//!
//! One short connection per call: read one message, answer it, close. Only the same user may
//! connect.

use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use terminescu_protocol::core::{CoreMessage, CoreReply, CoreRequest};
use terminescu_protocol::paths::{peer_uid, uid};
use terminescu_protocol::{APP_SUPPORTED, decode_message, encode_message};

/// Longest a caller may take to send its message.
const READ_TIMEOUT: Duration = Duration::from_secs(2);
/// Largest message accepted. Hook events carry tool input, which can be a whole file.
const MAX_MESSAGE: usize = 4 * 1024 * 1024;

pub type Handler = Arc<dyn Fn(CoreMessage, &Caller) -> CoreReply + Send + Sync>;

/// The CLI on the other end of a call, for a handler that blocks (`edit --wait`).
pub struct Caller<'a>(&'a UnixStream);

impl Caller<'_> {
    /// Whether the CLI hung up, as when `git commit` got Ctrl-C. It sends nothing after its
    /// message, so anything readable is the end of the stream.
    pub fn gone(&self) -> bool {
        use std::os::fd::AsRawFd;
        let mut fd = libc::pollfd {
            fd: self.0.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd for the length of the call, no wait.
        let n = unsafe { libc::poll(&mut fd, 1, 0) };
        n > 0 && fd.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0
    }
}

/// Binds the socket and serves it on a background thread. Fails with `AddrInUse` when another
/// app already answers on it; a socket file nobody answers on is stale and gets replaced.
pub fn serve(path: &Path, handler: Handler) -> io::Result<()> {
    if path.exists() {
        if UnixStream::connect(path).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("another app serves {}", path.display()),
            ));
        }
        std::fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    std::thread::Builder::new()
        .name("core-socket".into())
        .spawn(move || {
            let me = uid();
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                if peer_uid(&stream).ok() != Some(me) {
                    eprintln!("terminescu: refused a core socket connection from another user");
                    continue;
                }
                let handler = handler.clone();
                let _ = std::thread::Builder::new()
                    .name("core-call".into())
                    .spawn(move || serve_call(stream, &handler));
            }
        })?;
    Ok(())
}

fn serve_call(mut stream: UnixStream, handler: &Handler) {
    let message = match read_message(&mut stream) {
        Ok(m) => m,
        Err(e) => {
            let _ = stream.write_all(&encode_message(&CoreReply::error("bad_message", e)));
            return;
        }
    };
    if !APP_SUPPORTED.contains(&message.version) {
        let reply = CoreReply::error(
            "version",
            format!(
                "this CLI speaks version {}; the app speaks {:?}",
                message.version, APP_SUPPORTED
            ),
        );
        let _ = stream.write_all(&encode_message(&reply));
        return;
    }
    let is_hook = matches!(message.request, CoreRequest::Hook { .. });
    let reply = handler(message, &Caller(&stream));
    // The hook has already gone; it never waits for an answer.
    if !is_hook {
        let _ = stream.write_all(&encode_message(&reply));
    }
}

fn read_message(stream: &mut UnixStream) -> Result<CoreMessage, String> {
    stream
        .set_read_timeout(Some(READ_TIMEOUT))
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        match decode_message::<CoreMessage>(&buf) {
            Ok(Some((m, _))) => return Ok(m),
            Ok(None) => {}
            Err(e) => return Err(e.to_string()),
        }
        if buf.len() > MAX_MESSAGE {
            return Err("message too large".into());
        }
        match stream.read(&mut chunk) {
            Ok(0) => return Err("closed before a whole message".into()),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use terminescu_protocol::core::{Expiry, post, request};

    fn socket(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("tcs-{}-{name}.sock", std::process::id()))
    }

    #[test]
    fn answers_requests_and_takes_hooks_without_answering() {
        let path = socket("a");
        let seen: Arc<Mutex<Vec<CoreMessage>>> = Arc::default();
        let s = seen.clone();
        serve(
            &path,
            Arc::new(move |m, _| {
                s.lock().unwrap().push(m);
                CoreReply::Ok
            }),
        )
        .unwrap();
        let status = CoreMessage::new(
            3,
            CoreRequest::Status {
                label: Some("Deploying".into()),
                color: None,
                expiry: Expiry::Never,
            },
        );
        assert_eq!(
            request(&path, &status, Duration::from_secs(1)).unwrap(),
            CoreReply::Ok
        );
        let hook = CoreMessage::new(
            3,
            CoreRequest::Hook {
                event: serde_json::json!({"hook_event_name": "Stop"}),
            },
        );
        post(&path, &hook, Duration::from_millis(100)).unwrap();
        let start = std::time::Instant::now();
        while seen.lock().unwrap().len() < 2 {
            assert!(start.elapsed() < Duration::from_secs(2));
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(seen.lock().unwrap()[1], hook);

        // A second app steps aside; a stale file gets replaced.
        let again = serve(&path, Arc::new(|_, _| CoreReply::Ok));
        assert_eq!(again.unwrap_err().kind(), io::ErrorKind::AddrInUse);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_stale_socket_file_is_replaced() {
        let path = socket("b");
        drop(UnixListener::bind(&path).unwrap());
        assert!(path.exists());
        serve(&path, Arc::new(|_, _| CoreReply::error("x", "y"))).unwrap();
        let m = CoreMessage::new(
            1,
            CoreRequest::VarSet {
                name: "A".into(),
                value: "1".into(),
            },
        );
        assert_eq!(
            request(&path, &m, Duration::from_secs(1)).unwrap(),
            CoreReply::error("x", "y")
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_blocking_handler_sees_the_cli_hang_up() {
        let path = socket("c");
        let gone = Arc::new(Mutex::new(None));
        let g = gone.clone();
        serve(
            &path,
            Arc::new(move |_, caller| {
                let start = std::time::Instant::now();
                let still_there = !caller.gone();
                while !caller.gone() && start.elapsed() < Duration::from_secs(3) {
                    std::thread::sleep(Duration::from_millis(10));
                }
                *g.lock().unwrap() = Some((still_there, caller.gone()));
                CoreReply::Ok
            }),
        )
        .unwrap();
        let m = CoreMessage::new(
            1,
            CoreRequest::Edit {
                path: "/tmp/COMMIT_EDITMSG".into(),
                wait: true,
            },
        );
        // The CLI gives up after 100 ms and closes its end.
        assert!(request(&path, &m, Duration::from_millis(100)).is_err());
        let start = std::time::Instant::now();
        while gone.lock().unwrap().is_none() {
            assert!(start.elapsed() < Duration::from_secs(4));
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(*gone.lock().unwrap(), Some((true, true)));
        let _ = std::fs::remove_file(&path);
    }
}

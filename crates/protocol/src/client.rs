//! A blocking client for the daemon socket, used by the CLI and by tests. The app's client
//! lives in `terminescu-core` (task 2.6).

use crate::{APP_SUPPORTED, Control, DecodeError, Frame, decode, encode};
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Decode(#[from] DecodeError),
    #[error("daemon closed the connection")]
    Closed,
    #[error("daemon refused: {code}: {message}")]
    Refused { code: String, message: String },
    #[error("unexpected message: {0:?}")]
    Unexpected(Box<Frame>),
}

pub struct Client {
    stream: UnixStream,
    buf: Vec<u8>,
    pub version: u32,
    /// The daemon process this client talks to; see `Control::Welcome`.
    pub instance: String,
}

impl Client {
    /// Connects and completes the handshake.
    pub fn connect(socket: &Path, client: &str) -> Result<Self, ClientError> {
        Self::handshake(UnixStream::connect(socket)?, client)
    }

    /// Like `connect`, but every read and write, the handshake included, gives up after
    /// `timeout`. For callers that must not hang on a wedged daemon, such as `var env` in a
    /// prompt.
    pub fn connect_timeout(
        socket: &Path,
        client: &str,
        timeout: Duration,
    ) -> Result<Self, ClientError> {
        let stream = UnixStream::connect(socket)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        Self::handshake(stream, client)
    }

    fn handshake(stream: UnixStream, client: &str) -> Result<Self, ClientError> {
        let mut c = Self {
            stream,
            buf: Vec::new(),
            version: 0,
            instance: String::new(),
        };
        c.send(&Frame::Control(Control::Hello {
            versions: APP_SUPPORTED.collect(),
            client: client.into(),
        }))?;
        match c.recv()? {
            Frame::Control(Control::Welcome {
                version, instance, ..
            }) => {
                c.version = version;
                c.instance = instance;
            }
            Frame::Control(Control::Error { code, message, .. }) => {
                return Err(ClientError::Refused { code, message });
            }
            other => return Err(ClientError::Unexpected(Box::new(other))),
        }
        Ok(c)
    }

    /// The socket and any bytes already read past the handshake, for a caller that runs its own
    /// reader thread.
    pub fn into_parts(self) -> (UnixStream, Vec<u8>) {
        (self.stream, self.buf)
    }

    pub fn set_read_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        self.stream.set_read_timeout(t)
    }

    pub fn send(&mut self, frame: &Frame) -> Result<(), ClientError> {
        self.stream.write_all(&encode(frame))?;
        Ok(())
    }

    pub fn recv(&mut self) -> Result<Frame, ClientError> {
        loop {
            if let Some((frame, used)) = decode(&self.buf)? {
                self.buf.drain(..used);
                return Ok(frame);
            }
            let mut chunk = [0u8; 64 * 1024];
            let n = self.stream.read(&mut chunk)?;
            if n == 0 {
                return Err(ClientError::Closed);
            }
            self.buf.extend_from_slice(&chunk[..n]);
        }
    }

    /// Reads frames until `want` returns a value, passing over the others.
    pub fn recv_until<T>(
        &mut self,
        mut want: impl FnMut(&Frame) -> Option<T>,
    ) -> Result<T, ClientError> {
        loop {
            let f = self.recv()?;
            if let Some(v) = want(&f) {
                return Ok(v);
            }
        }
    }
}

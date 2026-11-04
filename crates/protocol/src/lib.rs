//! Wire format between the app and `terminescu-ptyd` (D7, D32).
//!
//! A frame is a big-endian `u32` length, then a `u8` kind, then the body. The length counts
//! the kind byte and the body. Kinds: 0 is a JSON control message, 1 is pane output and 2 is
//! pane input; both pane kinds start the body with a big-endian `u32` pane id.

use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;

/// The protocol version this build speaks natively.
pub const PROTOCOL_VERSION: u32 = 1;

/// Versions the app accepts from a running daemon: its own and the one before (D32). Version 1
/// has no predecessor; the lower bound becomes `PROTOCOL_VERSION - 1` with the first bump.
pub const APP_SUPPORTED: RangeInclusive<u32> = 1..=PROTOCOL_VERSION;

/// Largest frame either side accepts. Output batches stay far below it (D33 caps them at 256 KB).
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

const KIND_CONTROL: u8 = 0;
const KIND_OUTPUT: u8 = 1;
const KIND_INPUT: u8 = 2;

pub type PaneId = u32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Control(Control),
    Output { pane: PaneId, data: Vec<u8> },
    Input { pane: PaneId, data: Vec<u8> },
}

/// Control messages. The first message on every connection is `Hello` from the client,
/// answered by `Welcome` or `Error` (D32).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Control {
    Hello { versions: Vec<u32>, client: String },
    Welcome { version: u32, daemon: String },
    Error { code: String, message: String },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("frame of {0} bytes exceeds the limit")]
    TooLarge(usize),
    #[error("empty frame")]
    Empty,
    #[error("unknown frame kind {0}")]
    UnknownKind(u8),
    #[error("pane frame shorter than its pane id")]
    ShortPaneFrame,
    #[error("bad control message: {0}")]
    BadControl(String),
}

pub fn encode(frame: &Frame) -> Vec<u8> {
    let (kind, body): (u8, Vec<u8>) = match frame {
        Frame::Control(c) => (KIND_CONTROL, serde_json::to_vec(c).expect("control serializes")),
        Frame::Output { pane, data } => (KIND_OUTPUT, pane_body(*pane, data)),
        Frame::Input { pane, data } => (KIND_INPUT, pane_body(*pane, data)),
    };
    let len = u32::try_from(body.len() + 1).expect("frame fits u32");
    let mut out = Vec::with_capacity(body.len() + 5);
    out.extend_from_slice(&len.to_be_bytes());
    out.push(kind);
    out.extend_from_slice(&body);
    out
}

fn pane_body(pane: PaneId, data: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(data.len() + 4);
    b.extend_from_slice(&pane.to_be_bytes());
    b.extend_from_slice(data);
    b
}

/// Decodes one frame from the start of `buf`. Returns `Ok(None)` when `buf` holds only part
/// of a frame, otherwise the frame and the number of bytes it used.
pub fn decode(buf: &[u8]) -> Result<Option<(Frame, usize)>, DecodeError> {
    if buf.len() < 4 {
        return Ok(None);
    }
    let len = u32::from_be_bytes(buf[..4].try_into().unwrap()) as usize;
    if len == 0 {
        return Err(DecodeError::Empty);
    }
    if len > MAX_FRAME {
        return Err(DecodeError::TooLarge(len));
    }
    if buf.len() < 4 + len {
        return Ok(None);
    }
    let kind = buf[4];
    let body = &buf[5..4 + len];
    let frame = match kind {
        KIND_CONTROL => Frame::Control(
            serde_json::from_slice(body).map_err(|e| DecodeError::BadControl(e.to_string()))?,
        ),
        KIND_OUTPUT | KIND_INPUT => {
            if body.len() < 4 {
                return Err(DecodeError::ShortPaneFrame);
            }
            let pane = u32::from_be_bytes(body[..4].try_into().unwrap());
            let data = body[4..].to_vec();
            if kind == KIND_OUTPUT { Frame::Output { pane, data } } else { Frame::Input { pane, data } }
        }
        k => return Err(DecodeError::UnknownKind(k)),
    };
    Ok(Some((frame, 4 + len)))
}

/// Picks the version both sides speak: the highest version the client offers that the daemon
/// supports. `None` means the app must offer "Restart sessions" (D32).
pub fn negotiate(client_offers: &[u32], daemon_supports: &RangeInclusive<u32>) -> Option<u32> {
    client_offers.iter().copied().filter(|v| daemon_supports.contains(v)).max()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(f: Frame) {
        let bytes = encode(&f);
        let (back, used) = decode(&bytes).unwrap().unwrap();
        assert_eq!(back, f);
        assert_eq!(used, bytes.len());
    }

    #[test]
    fn frames_roundtrip() {
        roundtrip(Frame::Control(Control::Hello { versions: vec![1], client: "test".into() }));
        roundtrip(Frame::Output { pane: 7, data: b"\x1b[31mred\x1b[0m".to_vec() });
        roundtrip(Frame::Input { pane: u32::MAX, data: vec![] });
    }

    #[test]
    fn partial_frame_waits_for_more() {
        let bytes = encode(&Frame::Output { pane: 1, data: vec![b'x'; 100] });
        for cut in 0..bytes.len() {
            assert_eq!(decode(&bytes[..cut]).unwrap(), None, "cut at {cut}");
        }
    }

    #[test]
    fn two_frames_in_one_buffer() {
        let mut bytes = encode(&Frame::Input { pane: 1, data: b"ls\r".to_vec() });
        let first = bytes.len();
        bytes.extend(encode(&Frame::Input { pane: 2, data: b"pwd\r".to_vec() }));
        let (_, used) = decode(&bytes).unwrap().unwrap();
        assert_eq!(used, first);
        let (f, _) = decode(&bytes[used..]).unwrap().unwrap();
        assert_eq!(f, Frame::Input { pane: 2, data: b"pwd\r".to_vec() });
    }

    #[test]
    fn rejects_bad_frames() {
        assert_eq!(decode(&[0, 0, 0, 0]), Err(DecodeError::Empty));
        let huge = ((MAX_FRAME + 1) as u32).to_be_bytes();
        assert_eq!(decode(&huge), Err(DecodeError::TooLarge(MAX_FRAME + 1)));
        assert_eq!(decode(&[0, 0, 0, 1, 9]), Err(DecodeError::UnknownKind(9)));
        assert_eq!(decode(&[0, 0, 0, 3, 1, 0, 0]), Err(DecodeError::ShortPaneFrame));
        assert!(matches!(decode(&[0, 0, 0, 2, 0, b'{']), Err(DecodeError::BadControl(_))));
    }

    #[test]
    fn control_json_shape() {
        let json = serde_json::to_string(&Control::Welcome { version: 1, daemon: "0.1.0".into() }).unwrap();
        assert_eq!(json, r#"{"op":"welcome","version":1,"daemon":"0.1.0"}"#);
    }

    #[test]
    fn negotiation_prefers_highest_shared_version() {
        assert_eq!(negotiate(&[1, 2], &(1..=1)), Some(1));
        assert_eq!(negotiate(&[1, 2], &(1..=2)), Some(2));
        assert_eq!(negotiate(&[3], &(1..=2)), None);
        assert!(APP_SUPPORTED.contains(&PROTOCOL_VERSION));
    }
}

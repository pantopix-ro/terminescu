//! A file read as it grows, one complete line at a time (D34). The core tails an agent's log
//! with it and hands each line to the agent's adapter; an adapter parses the lines itself.

use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// Longest stretch read in one call; a file that grew more catches up over later reads.
const MAX_READ: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct LineTail {
    path: PathBuf,
    offset: u64,
    inode: Option<u64>,
    /// Inside a line longer than [`MAX_READ`]: drop bytes up to its end.
    skipping: bool,
}

impl LineTail {
    /// From the start of the file.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            offset: 0,
            inode: None,
            skipping: false,
        }
    }

    /// From the file's current end, for watching what comes next. A missing file starts at 0.
    pub fn from_end(path: impl Into<PathBuf>) -> Self {
        let mut tail = Self::new(path);
        if let Ok(meta) = std::fs::metadata(&tail.path) {
            tail.offset = meta.len();
            tail.inode = Some(meta.ino());
        }
        tail
    }

    /// From a byte offset an earlier read returned, for a reader that keeps its own cursor.
    pub fn at(path: impl Into<PathBuf>, offset: u64) -> Self {
        let mut tail = Self::new(path);
        tail.offset = offset;
        tail
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Every complete line added since the last read, with its byte offset, without the
    /// newline. A file that shrank or was replaced is read again from the start.
    pub fn read(&mut self) -> std::io::Result<Vec<(u64, Vec<u8>)>> {
        let mut file = match std::fs::File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let meta = file.metadata()?;
        if meta.len() < self.offset || self.inode.is_some_and(|i| i != meta.ino()) {
            self.offset = 0;
            self.skipping = false;
        }
        self.inode = Some(meta.ino());
        file.seek(SeekFrom::Start(self.offset))?;
        let mut buf = Vec::new();
        file.take(MAX_READ as u64).read_to_end(&mut buf)?;
        let Some(last) = buf.iter().rposition(|&b| b == b'\n') else {
            if buf.len() == MAX_READ {
                // One line fills the whole read: skip it rather than stall here.
                self.offset += buf.len() as u64;
                self.skipping = true;
            }
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        let mut start = 0;
        for (i, &b) in buf[..=last].iter().enumerate() {
            if b != b'\n' {
                continue;
            }
            if self.skipping {
                self.skipping = false;
            } else {
                out.push((self.offset + start as u64, buf[start..i].to_vec()));
            }
            start = i + 1;
        }
        self.offset += last as u64 + 1;
        Ok(out)
    }
}

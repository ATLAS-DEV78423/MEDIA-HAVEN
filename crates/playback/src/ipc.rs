//! Line-delimited JSON IPC channel to mpv (ADR-0002), behind a trait so the
//! player can be tested with [`FakeIpc`] and no mpv binary or display.
//!
//! mpv's `--input-ipc-server` is a Unix domain socket on Unix and a Windows
//! named pipe on Windows. The client writes one JSON object per line and reads
//! one JSON object per line. [`MpvIpc`] is the real transport; [`FakeIpc`] is
//! the in-memory test double used by the player tests.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::time::{Duration, Instant};

/// Errors from the IPC channel itself. Command-level errors reported by mpv
/// (a reply with `"error": "..."`) are handled by the player, not here.
#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("ipc io: {0}")]
    Io(#[from] std::io::Error),
    #[error("mpv IPC disconnected before a reply")]
    Disconnected,
    #[error("no canned reply left in the fake transport")]
    NoReply,
}

/// A newline-framed, bidirectional byte channel carrying mpv JSON messages.
pub trait IpcTransport {
    /// Write one complete message plus a trailing newline.
    fn write_line(&mut self, line: &str) -> Result<(), IpcError>;
    /// Read one message, with the trailing newline stripped.
    fn read_line(&mut self) -> Result<String, IpcError>;
}

/// Real transport holding two handles to the same mpv IPC channel: a buffered
/// reader and a writer. Both handles are owned so `read_line`/`write_line` can
/// borrow `&mut self` independently.
pub struct MpvIpc<R> {
    reader: BufReader<R>,
    writer: R,
}

impl<R: Read + Write> MpvIpc<R> {
    fn new(reader: R, writer: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            writer,
        }
    }
}

impl<R: Read + Write> IpcTransport for MpvIpc<R> {
    fn write_line(&mut self, line: &str) -> Result<(), IpcError> {
        self.writer.write_all(line.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        Ok(())
    }

    fn read_line(&mut self) -> Result<String, IpcError> {
        let mut buf = String::new();
        if self.reader.read_line(&mut buf)? == 0 {
            return Err(IpcError::Disconnected);
        }
        while buf.ends_with('\n') || buf.ends_with('\r') {
            buf.pop();
        }
        Ok(buf)
    }
}

/// The concrete stream type mpv's IPC uses on this host.
#[cfg(windows)]
pub type ConnectedIpc = MpvIpc<std::fs::File>;
#[cfg(not(windows))]
pub type ConnectedIpc = MpvIpc<std::os::unix::net::UnixStream>;

/// Connect to mpv's IPC endpoint, retrying until `timeout` elapses. mpv creates
/// the endpoint asynchronously after spawn, so a short retry loop implements
/// the required 5 s connect timeout without an extra dependency.
#[cfg(windows)]
pub fn connect(endpoint: &str, timeout: Duration) -> Result<ConnectedIpc, IpcError> {
    let deadline = Instant::now() + timeout;
    loop {
        // On Windows std::fs::OpenOptions wraps CreateFileW, which opens an
        // existing named pipe (`\\.\pipe\<name>`) as a read/write handle.
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(endpoint)
        {
            Ok(file) => {
                let writer = file.try_clone()?;
                return Ok(MpvIpc::new(file, writer));
            }
            Err(err) => {
                if Instant::now() >= deadline {
                    return Err(IpcError::Io(err));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// Connect to mpv's Unix domain socket, retrying until `timeout` elapses.
#[cfg(not(windows))]
pub fn connect(endpoint: &str, timeout: Duration) -> Result<ConnectedIpc, IpcError> {
    use std::os::unix::net::UnixStream;

    let deadline = Instant::now() + timeout;
    loop {
        match UnixStream::connect(endpoint) {
            Ok(stream) => {
                stream.set_read_timeout(Some(timeout))?;
                stream.set_write_timeout(Some(timeout))?;
                let writer = stream.try_clone()?;
                return Ok(MpvIpc::new(stream, writer));
            }
            Err(err) => {
                if Instant::now() >= deadline {
                    return Err(IpcError::Io(err));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// In-memory IPC double: records everything written and replays a queue of
/// canned reply lines (e.g. `{"error":"success","data":{...}}`).
#[derive(Debug, Default)]
pub struct FakeIpc {
    written: Vec<String>,
    replies: VecDeque<String>,
}

impl FakeIpc {
    pub fn with_replies<I: IntoIterator<Item = String>>(replies: I) -> Self {
        Self {
            written: Vec::new(),
            replies: replies.into_iter().collect(),
        }
    }

    /// The exact lines the player wrote, including the trailing newline.
    pub fn written(&self) -> &[String] {
        &self.written
    }
}

impl IpcTransport for FakeIpc {
    fn write_line(&mut self, line: &str) -> Result<(), IpcError> {
        self.written.push(line.to_string());
        Ok(())
    }

    fn read_line(&mut self) -> Result<String, IpcError> {
        self.replies.pop_front().ok_or(IpcError::NoReply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_replays_canned_replies_in_order() {
        let mut fake = FakeIpc::with_replies([
            r#"{"error":"success","data":{"x":1}}"#.to_string(),
            r#"{"error":"success","data":{}}"#.to_string(),
        ]);
        fake.write_line("{\"command\":[\"get_property\"]}\n")
            .unwrap();
        assert_eq!(
            fake.written(),
            &["{\"command\":[\"get_property\"]}\n".to_string()]
        );
        assert_eq!(
            fake.read_line().unwrap(),
            r#"{"error":"success","data":{"x":1}}"#
        );
        assert_eq!(
            fake.read_line().unwrap(),
            r#"{"error":"success","data":{}}"#
        );
        assert!(matches!(fake.read_line(), Err(IpcError::NoReply)));
    }
}

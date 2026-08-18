//! One request, one answer, then exit.
//!
//! Everything a command needs to know about the daemon is here: whether one is
//! running, how to start one, and how to ask it a question.

use super::{Answer, Request, socket_path};
use anyhow::{Context, Result, bail};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// Whether a daemon is listening.
///
/// Connecting rather than checking for the file, because a socket left behind by
/// a daemon that died is a file with nobody on the other end.
pub fn running() -> bool {
    UnixStream::connect(socket_path()).is_ok()
}

/// Sends one request, requiring a daemon to already be there.
pub fn ask(request: &Request) -> Result<Answer> {
    let path = socket_path();
    let stream = UnixStream::connect(&path).with_context(|| {
        format!(
            "no game is running (nothing listening at {}). `sts2 start` begins one",
            path.display()
        )
    })?;

    round_trip(stream, request)
}

/// Sends one request, starting a daemon first if there is not one.
pub fn ask_or_start(request: &Request, idle_timeout: Duration) -> Result<Answer> {
    if !running() {
        super::daemon::spawn_detached(idle_timeout)?;
    }

    ask(request)
}

fn round_trip(stream: UnixStream, request: &Request) -> Result<Answer> {
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);

    writeln!(writer, "{}", serde_json::to_string(request)?)?;
    writer.flush()?;

    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        bail!("the daemon closed the connection without answering");
    }

    serde_json::from_str(line.trim())
        .with_context(|| format!("reading the daemon's answer: {}", line.trim()))
}

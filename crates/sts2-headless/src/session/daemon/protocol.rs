//! What a client and a daemon say to each other.
//!
//! Not the game's protocol. [`crate::wire`] is what the *game* answers. This is
//! the thin envelope around it. It lets a short-lived process reach a game it
//! did not launch: one JSON line each way over a unix socket.
//!
//! Private, and meant to stay private. A caller drives a held game like any
//! other, and asks after the session itself
//! through [`Session`](crate::Session)'s own methods. Nothing outside this crate
//! has to know a socket is involved, which is the whole reason the daemon moved
//! in here.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// What a client asks the daemon to do.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Request {
    /// A game command, already encoded. The daemon relays it verbatim.
    ///
    /// Encoded on the client, where the build is named, so the daemon stays a
    /// relay and the typing happens once at the edge.
    Game {
        command: Value,
    },
    /// Write every decision from here on to a file, so the run can be replayed.
    ///
    /// Set on the daemon rather than passed with each command, because the
    /// daemon is the only thing that sees the whole run. A client is one process
    /// per command, so it has to be told the path every time. Any invocation
    /// that forgot leaves a gap in the file that nothing detects.
    Record {
        /// Where to write. Absent stops recording.
        path: Option<PathBuf>,
        /// Carry on writing an existing recording rather than starting one.
        ///
        /// What a replay does. The file already holds the steps just replayed,
        /// so rewriting it destroys the run to reproduce it. A replay that
        /// failed halfway leaves the file it was opened to investigate
        /// truncated at the failure.
        #[serde(default)]
        append: bool,
    },
    /// Anything about the session rather than the game.
    Status,
    Stop,
}

/// What the daemon answers.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Answer {
    /// The game's reply line, verbatim.
    ///
    /// Unparsed on purpose: the daemon is a relay, and decoding a reply only to
    /// re-encode it needs a `Serialize` inverse for every response type.
    /// The client parses it once, with the same [`crate::parse_reply`] an
    /// in-process [`Oracle`](crate::Oracle) uses.
    Reply {
        line: String,
        /// Something went wrong beside the command, which still succeeded.
        ///
        /// A recording the daemon failed to write is the case. The command was
        /// accepted and applied, so the reply is real and has to reach the
        /// caller. The file has stopped being a run, and saying nothing about
        /// that is how it gets trusted later.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        warning: Option<String>,
    },
    Status {
        pid: u32,
        game: String,
        idle_seconds: u64,
        idle_timeout_seconds: u64,
        /// Where the run is being written, if it is.
        #[serde(default)]
        recording: Option<PathBuf>,
    },
    /// A recording started or stopped.
    Recording {
        path: Option<PathBuf>,
        steps: usize,
    },
    Stopping,
    /// The daemon refused or failed: no run started, the game died, and so on.
    Failed {
        message: String,
    },
}

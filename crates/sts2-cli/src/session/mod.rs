//! One long-lived game, and short-lived commands against it.
//! The game takes seconds to boot and holds all the state. Therefore a command cannot be a whole process: `sts2 play-card 0` has to reach a game that is already running. A REPL is the smaller design and is the wrong one. Anything driving this from a script or an agent gets one process per command. Therefore it has to commit to every input before seeing any output.
//! So: a [`daemon`] owns the [`Game`](sts2_headless::Game) and listens on a unix socket. A [`client`] connects, sends one request, prints one answer and exits. The daemon shuts itself down after a period of quiet, because nothing else will, a client that crashes leaves no one to clean up.

pub mod client;
pub mod daemon;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

/// How long the daemon waits before deciding nobody is coming back.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Where the socket lives.
///
/// `XDG_RUNTIME_DIR` when there is one, because it is per-user, cleaned up at logout, and not world-writable. `/tmp` otherwise, with the uid in the name so two users on one machine do not collide.
pub fn socket_path() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) => PathBuf::from(dir).join("sts2.sock"),
        None => PathBuf::from(format!("/tmp/sts2-{}.sock", uid())),
    }
}

/// The user, for keeping two people's sockets apart under `/tmp`.
///
/// Read from the environment rather than pulled in with a `libc` dependency for one number. It is a name, not a permission: the socket's own mode is what stops anyone else connecting, and this only avoids a collision.
fn uid() -> String {
    std::env::var("UID")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "shared".into())
}

/// What a client asks the daemon to do.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    /// A game command, already encoded, the daemon relays it verbatim.
    ///
    /// Encoded on the client, where the content types are, so the daemon stays a relay and the typing happens once at the edge.
    Game {
        command: Value,
    },
    /// Anything about the session rather than the game.
    Status,
    Stop,
}

/// What the daemon answers.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Answer {
    /// The game's reply line, verbatim.
    ///
    /// Unparsed on purpose: the daemon is a relay, and decoding a reply only to re-encode it will need a `Serialize` inverse for every response type. The client parses it once, with the same code any other caller uses.
    Reply {
        line: String,
    },
    Status {
        pid: u32,
        game: String,
        idle_seconds: u64,
        idle_timeout_seconds: u64,
    },
    Stopping,
    /// The daemon cannot do it, no run started, the game died, and so on.
    Failed {
        message: String,
    },
}

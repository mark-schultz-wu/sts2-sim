//! What can go wrong between this process and the game.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No `SlayTheSpire2` at the path the config named.
    #[error("no game binary at {0}")]
    GameNotFound(PathBuf),

    /// The game exited before connecting back. Set
    /// [`crate::Config::inherit_game_output`] to see why.
    #[error("the game exited with status {0} before connecting")]
    GameExited(i32),

    /// The game was killed before connecting back.
    #[error("the game was killed before connecting")]
    GameKilled,

    /// Something connected to our port, but it was not our game.
    #[error("something other than the game connected to our port")]
    WrongPeer,

    /// The game's first message was not a greeting.
    #[error("expected a greeting, got a '{0}' message")]
    NoGreeting(String),

    /// The game is running a different build of the mod than this client was
    /// compiled against.
    ///
    /// Almost always means the mod was not redeployed after a source change.
    /// Refused at the handshake, because the alternative is a decode failure
    /// somewhere deep in a run, blamed on whatever was being played at the time.
    #[error(
        "the game is running mod {found}, this client was built against {expected}; redeploy the mod"
    )]
    WrongMod { expected: String, found: String },

    /// The connection closed mid-conversation.
    #[error("the game closed the connection")]
    GameGone,

    /// A deadline passed. The session is not usable afterwards.
    #[error("timed out {0}")]
    Timeout(&'static str),

    #[error("{0}: {1}")]
    Io(&'static str, #[source] std::io::Error),

    /// A command failed to encode.
    ///
    /// Holds the command's rendering rather than the command, so that [`Error`]
    /// stays free of the content-version parameter. An error type generic over
    /// the build puts that parameter in every signature that can fail, to carry
    /// a string nobody matches on.
    #[error("could not encode {0}: {1}")]
    Encode(String, #[source] serde_json::Error),

    /// A line arrived that this crate cannot read as a response.
    #[error("could not read a response: {1}; line was: {0}")]
    Decode(String, #[source] serde_json::Error),

    /// The game is not the build this client's content tables describe.
    ///
    /// Refused at launch rather than allowed to proceed. Every typed id this
    /// client can send comes from tables generated against one build, and
    /// driving another means every one of them is a guess.
    #[error("this client's tables describe {expected}, but the game is {found}")]
    WrongBuild { expected: String, found: String },

    /// The game answered, and this crate cannot read what it said.
    ///
    /// A fault and not a refusal: the game did move, and the client no longer
    /// knows where it moved to. Almost always a field the mod started sending
    /// that `wire::observation` has no home for. That is exactly the drift the
    /// strict mirror exists to stop rather than absorb.
    #[error("the game's answer did not parse: {0}")]
    Unreadable(#[from] crate::ObservationError),

    /// The adapter fell over inside the mod.
    ///
    /// Not a refusal, and the distinction is the whole reason the mod sends this
    /// as its own message type. A refusal is the game's answer -- the move was
    /// illegal, deterministically, and a simulator must give the same one. This
    /// is the mod failing to ask the question at all, and there is nothing for a
    /// simulator to agree with.
    #[error("the mod threw {error} handling {cmd}: {message}")]
    Faulted {
        cmd: String,
        /// The exception's type name.
        error: String,
        message: String,
    },

    /// Nothing is listening where a held game belongs.
    ///
    /// Its own variant rather than an [`Error::Io`], because it is the ordinary
    /// case rather than a fault. No game has been started yet, and the caller is
    /// the one that knows what to tell a person about that.
    #[error("no game is running (nothing listening at {0})")]
    NoDaemon(std::path::PathBuf),

    /// The daemon holding the game turned the request down.
    ///
    /// Beside a [`sts2_core::Refusal`] and not the same thing. A refusal is the
    /// *game* declining a move. This is the session around it declining to carry
    /// the question at all. No run started, a recording that failed to open, a
    /// request from a client of another build.
    #[error("{0}")]
    Daemon(String),

    /// The daemon answered something other than what was asked.
    #[error("unexpected answer to {asked}: {got}")]
    Unexpected { asked: &'static str, got: String },

    /// The game said goodbye where an answer was due.
    ///
    /// `Bye` is the reply to a quit, and the typed API sends none. Dropping the
    /// [`crate::Oracle`] is what ends a session. So one arriving here means
    /// something else on this socket asked the game to leave.
    #[error("the game said goodbye in answer to a command")]
    SaidGoodbye,
}

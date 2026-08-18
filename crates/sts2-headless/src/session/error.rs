//! What can go wrong between this process and the game.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No `SlayTheSpire2` where the config said it would be.
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

    /// The game speaks a protocol this crate does not.
    #[error("the game speaks protocol v{found}, this client speaks v{expected}")]
    ProtocolMismatch { expected: u32, found: u32 },

    /// The connection closed mid-conversation.
    #[error("the game closed the connection")]
    GameGone,

    /// A deadline passed. The session is not usable afterwards.
    #[error("timed out {0}")]
    Timeout(&'static str),

    #[error("{0}: {1}")]
    Io(&'static str, #[source] std::io::Error),

    /// A command could not be encoded.
    ///
    /// Holds the command's rendering rather than the command, so that [`Error`]
    /// stays free of the content-version parameter. An error type generic over
    /// the build would put that parameter in every signature that can fail, to
    /// carry a string nobody matches on.
    #[error("could not encode {0}: {1}")]
    Encode(String, #[source] serde_json::Error),

    /// A line arrived that this crate could not read as a response.
    #[error("could not read a response: {1}; line was: {0}")]
    Decode(String, #[source] serde_json::Error),

    /// The game is not the build this client's content tables describe.
    ///
    /// Refused at launch rather than allowed to proceed: every typed id this
    /// client can send comes from tables generated against one build, and
    /// driving another means every one of them is a guess.
    #[error("this client's tables describe {expected}, but the game is {found}")]
    WrongBuild { expected: String, found: String },
}

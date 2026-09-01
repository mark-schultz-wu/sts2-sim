//! Where the game is and how it must be launched.

use std::path::PathBuf;
use std::time::Duration;

/// The game's executable, inside [`Config::game_dir`].
pub(crate) const BINARY: &str = "SlayTheSpire2";

/// How to launch the game.
#[derive(Debug, Clone)]
pub struct Config {
    /// The directory holding the `SlayTheSpire2` binary. Also the working directory, which the game expects.
    pub game_dir: PathBuf,
    /// Let the game's own output through to this process's stdout and stderr.
    ///
    /// Off by default, and that matters: the engine writes ~140 unprefixed lines during startup, which will scribble over a TUI. Turn it on to debug a launch that is failing.
    pub inherit_game_output: bool,
    /// How long to wait for the game to start and connect back.
    pub startup_timeout: Duration,
    /// How long to wait for a reply to one command.
    pub command_timeout: Duration,
}

impl Config {
    pub fn new(game_dir: impl Into<PathBuf>) -> Self {
        Self {
            game_dir: game_dir.into(),
            inherit_game_output: false,
            // Measured at ~3s from spawn to a connected game, the ~10s the
            // mod README quotes is startup to the main menu, which serve mode
            // never reaches. The slack is for a loaded machine, not for a game
            // that is never going to arrive.
            startup_timeout: Duration::from_secs(60),
            command_timeout: Duration::from_secs(30),
        }
    }

    /// Let the game's output through. See [`Config::inherit_game_output`].
    pub fn inherit_game_output(mut self, yes: bool) -> Self {
        self.inherit_game_output = yes;
        self
    }
}

/// Where the game is, if the environment says.
///
/// `STS2_GAME_DIR` is the same variable the justfile and `deploy.sh` read. Therefore a shell set up to build the mod is already set up to launch it.
pub fn game_dir_from_env() -> Option<PathBuf> {
    std::env::var_os("STS2_GAME_DIR").map(PathBuf::from)
}

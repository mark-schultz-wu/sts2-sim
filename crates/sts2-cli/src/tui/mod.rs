//! The TUI: one process, holding the game open, repainting after every move.
//!
//! The opposite trade to the subcommands next door, and both are wanted. A
//! command per process is what lets a script or an agent send one move, read
//! the answer and decide the next. It is a poor way for a person to play,
//! because every look costs a line of typing and nothing stays on the screen.
//! This holds the game between moves instead, so the screen *is* the state.
//!
//! What it knows about the game is what the subcommands know: that it
//! implements [`Play`](sts2_core::Play). The type it drives is named once, in
//! `main.rs`, and a simulator goes in the same slot.
//!
//! Nothing here decides anything about the run. Every move goes through
//! `Decision<V>`, and every screen is drawn from the [`View`](sts2_core::View)
//! of what came back. The TUI cannot show a state the game did not report, nor
//! one the game keeps off its own screen. The failure mode is a blank panel,
//! never an invented number.

pub mod app;
pub mod check;
mod keys;
mod names;
mod screen;
pub mod setup;
#[cfg(test)]
mod shots;
mod state;
mod theme;

use ratatui::DefaultTerminal;
use sts2_core::Play;

pub use app::Ended;

/// Why the screen closed on anything but the player leaving.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("drawing the {screen}: {source}")]
    Draw {
        screen: &'static str,
        source: std::io::Error,
    },
    #[error("reading a key: {0}")]
    Key(std::io::Error),
    #[error("asking the game where it is: {0}")]
    Looking(Box<dyn std::error::Error + Send + Sync>),
    #[error("sending {said}: {source}")]
    Sending {
        said: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
pub use state::Options;

/// Draws a run and reads keys, until the player leaves or the run goes away.
///
/// A window onto a run that has already started. What run that is, and what to
/// do once this returns, belong to the caller: this draws one and reports how
/// it ended.
pub fn play<V, G>(
    terminal: &mut DefaultTerminal,
    game: G,
    options: Options,
) -> Result<app::Ended, Error>
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    app::App::playing(game, options).run(terminal)
}

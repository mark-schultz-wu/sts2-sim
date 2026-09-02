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
//! `Decision<V>`, and every screen is drawn from the `Observation<V>` that came
//! back. The TUI cannot show a state the game did not report. The failure mode
//! is a blank panel, never an invented number.

pub mod app;
pub mod check;
mod keys;
mod names;
mod screen;
#[cfg(test)]
mod shots;
mod state;
mod theme;

use anyhow::Result;
use sts2_core::{ContentVersion, Play};

pub use state::Options;

/// Plays until the user leaves.
///
/// The terminal is put into raw mode and an alternate screen here, and restored
/// on the way out, including out of a panic. `ratatui::init` installs the hook
/// that does it, so a crash leaves a usable terminal behind rather than a dead
/// one.
pub fn run<V, G>(game: G, options: Options) -> Result<()>
where
    V: ContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let mut terminal = ratatui::init();
    let outcome = app::App::new(game, options).run(&mut terminal);
    ratatui::restore();
    outcome
}

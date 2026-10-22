//! Slay the Spire 2 in the terminal, on the simulator.
//!
//! The screen `sts2` draws, over [`sts2_sim::Sim`] rather than the game. There
//! is no game to boot and no daemon to hold it: the run lives in this process,
//! and leaving ends it.
//!
//! ```text
//! sts2-sim
//! sts2-sim --game-version v0.111.0
//! ```
//!
//! The setup screen asks for a seed and a character, and the run starts at
//! once. When a run ends the setup screen comes back for the next. A path the
//! simulator has no port for panics, and the panic names it.

use std::process::ExitCode;

use clap::Parser;
use sts2_cli::tui;
use sts2_core::{Play as _, Rules};
use sts2_sim::Sim;

/// Play a run on the simulator.
#[derive(Debug, Parser)]
#[command(
    name = "sts2-sim",
    about = "Play Slay the Spire 2 on the simulator.",
    version
)]
struct Cli {
    /// Which build's rules to play, as its lock under `mod/games/` names it.
    #[arg(long, default_value = "v0.107.1")]
    game_version: String,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Screen(#[from] tui::Error),
    #[error("no build {0:?}; try v0.107.1 or v0.111.0")]
    NoBuild(String),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // One arm per build `sts2-content` compiles.
    let played = match cli.game_version.as_str() {
        "v0.107.1" => play::<sts2_content::v0_107_1::V0_107_1>(),
        "v0.111.0" => play::<sts2_content::v0_111_0::V0_111_0>(),
        other => Err(Error::NoBuild(other.to_owned())),
    };
    match played {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The setup screen, then a run on it, until the player quits.
fn play<V: sts2_content_wire::WireContentVersion + Rules>() -> Result<(), Error> {
    ratatui::run(|terminal| {
        let mut screen = tui::setup::Screen::<V>::default();
        loop {
            let Some(start) = screen.collect(terminal)? else {
                return Ok(());
            };
            let game = match Sim::<V>::begin((), &start) {
                Ok(game) => game,
                Err(why) => {
                    screen.refused(why.to_string());
                    continue;
                }
            };
            let options = tui::Options { recording: None };
            if tui::play(terminal, game, options)? == tui::Ended::Quit {
                return Ok(());
            }
        }
    })
}

//! Repo chores with real logic in them. `cargo xtask <command>`.
//!
//! `dump` runs the game's dump modes and files what they print. `codegen` turns
//! those dumps in `sts2-content/src/<version>/dumps/` into that crate's tables,
//! one domain per dump, and fails the build if two dumps came from different
//! builds.
//!
//! Both commands work on the build `STS2_GAME_VERSION` names, which is what
//! `.envrc` exports and what `mod/build.sh` compiles the mods against.
//!
//! The generated tree is committed rather than produced by a `build.rs`, so that
//! a content change reads as a diff. The whole point of this project is noticing
//! when the game and the simulator disagree. "Which 40 cards did that patch
//! touch" is worth being able to see.

mod codegen;
mod dump;

use std::process::ExitCode;

fn main() -> ExitCode {
    // Everything after the command name. `dump` reads them as mode names.
    let args: Vec<String> = std::env::args().skip(2).collect();

    match std::env::args().nth(1).as_deref() {
        // Named modes, or all of them when none is named.
        Some("dump") => match dump::run(&args) {
            Ok(summary) => {
                println!("{summary}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("dump failed: {e}");
                ExitCode::FAILURE
            }
        },
        Some("codegen") => match codegen::run() {
            Ok(summary) => {
                println!("{summary}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("codegen failed: {e}");
                ExitCode::FAILURE
            }
        },
        Some(other) => {
            eprintln!("unknown command '{other}'; try: dump, codegen");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("usage: cargo xtask <command>\n\ncommands:\n  dump [MODE...]\n  codegen");
            ExitCode::FAILURE
        }
    }
}

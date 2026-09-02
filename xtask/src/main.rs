//! Repo chores with real logic in them. `cargo xtask <command>`.
//! `dump` runs the game's dump modes and files what they print. `codegen` turns those dumps into `sts2-content`'s tables, one domain per dump. A build failure if two dumps came from different builds.
//! The generated tree is committed rather than produced by a `build.rs`, because a content change must be readable as a diff. In a project whose whole point is noticing when the game and the simulator disagree. "which 40 cards did that patch touch" is worth being able to see.

mod codegen;
mod dump;

use std::process::ExitCode;

fn main() -> ExitCode {
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

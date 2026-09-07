//! Repo chores with real logic in them. `cargo xtask <command>`.
//!
//! Two commands. `codegen` turns the dumps in
//! `sts2-content/src/<version>/dumps/` into that crate's tables, one domain per
//! dump, and fails the build if two dumps came from different builds.
//!
//! The generated tree is committed rather than produced by a `build.rs`, so that
//! a content change reads as a diff. The whole point of this project is noticing
//! when the game and the simulator disagree. "Which 40 cards did that patch
//! touch" is worth being able to see.

mod codegen;
mod dump;

use std::process::ExitCode;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
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
        // Every mode when none is named, which is what `just dump` does.
        Some("dump") => match dump::run(&std::env::args().skip(2).collect::<Vec<_>>()) {
            Ok(summary) => {
                println!("{summary}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("dump failed: {e}");
                ExitCode::FAILURE
            }
        },
        Some(other) => {
            eprintln!("unknown command '{other}'; try: codegen, dump");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("usage: cargo xtask <command>\n\ncommands:\n  codegen\n  dump");
            ExitCode::FAILURE
        }
    }
}

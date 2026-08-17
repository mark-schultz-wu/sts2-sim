//! Repo chores with real logic in them. `cargo xtask <command>`.
//!
//! Currently one command: `codegen`, which turns the dumps in
//! `sts2-content/src/<version>/dumps/` into that crate's tables — one domain
//! per dump, and a build failure if two dumps came from different builds.
//!
//! The generated tree is committed rather than produced by a `build.rs`,
//! because a content change should be readable as a diff. In a project whose
//! whole point is noticing when the game and the simulator disagree, "which 40
//! cards did that patch touch" is worth being able to see.

mod codegen;

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
        Some(other) => {
            eprintln!("unknown command '{other}'; try: codegen");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("usage: cargo xtask <command>\n\ncommands:\n  codegen");
            ExitCode::FAILURE
        }
    }
}

//! Repo chores with real logic in them. `cargo xtask <command>`.
//!
//! Four commands. `codegen` turns the dumps in
//! `sts2-content/src/<version>/dumps/` into that crate's tables, one domain per
//! dump, and fails the build if two dumps came from different builds. `dump`
//! asks the game for those dumps. `instructions` counts what a replayed step
//! costs, and `trace` shows where it goes, span by span.
//!
//! The generated tree is committed rather than produced by a `build.rs`, so that
//! a content change reads as a diff. The whole point of this project is noticing
//! when the game and the simulator disagree. "Which 40 cards did that patch
//! touch" is worth being able to see.

mod codegen;
mod dump;
mod instructions;
mod trace;

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
        Some("instructions") => {
            match instructions::run(&std::env::args().skip(2).collect::<Vec<_>>()) {
                Ok(report) => {
                    print!("{report}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("instructions failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("trace") => match trace::run(&std::env::args().skip(2).collect::<Vec<_>>()) {
            Ok(report) => {
                print!("{report}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("trace failed: {e}");
                ExitCode::FAILURE
            }
        },
        Some(other) => {
            eprintln!("unknown command '{other}'; try: codegen, dump, instructions, trace");
            ExitCode::FAILURE
        }
        None => {
            eprintln!(
                "usage: cargo xtask <command>\n\ncommands:\n  codegen\n  dump\n  instructions\n  trace"
            );
            ExitCode::FAILURE
        }
    }
}

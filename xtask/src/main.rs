//! Repo chores with real logic in them. `cargo xtask <command>`.
//!
//! Seven commands. `codegen` turns the dumps in
//! `sts2-content/src/<version>/dumps/` into that crate's tables, one domain per
//! dump, and fails the build if two dumps came from different builds. `dump`
//! asks the game for those dumps. `instructions` counts what a replayed step
//! costs, and `trace` shows where it goes, span by span. `allocations` names
//! where it reaches the heap. `census` ranks what stops random runs, and
//! `neow` finds the seeds whose Neow offers a relic.
//!
//! The generated tree is committed rather than produced by a `build.rs`, so that
//! a content change reads as a diff. The whole point of this project is noticing
//! when the game and the simulator disagree. "Which 40 cards did that patch
//! touch" is worth being able to see.

mod allocations;
mod asm;
mod census;
mod codegen;
mod dump;
mod instructions;
mod neow;
mod plays;
mod trace;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "cargo xtask",
    bin_name = "cargo xtask",
    about = "Repo chores with real logic in them."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate the content tables from the dumps.
    Codegen,
    /// Ask the game for its dumps.
    Dump(dump::Args),
    /// Count what a replayed step costs, in instructions.
    Instructions(instructions::Args),
    /// Name where a replayed step reaches the heap.
    Allocations(allocations::Args),
    /// Play runs at random and rank what stops them.
    Census(census::Args),
    /// Find the seeds whose Neow offers a relic.
    Neow(neow::Args),
    /// Show where a replayed step's instructions go, span by span.
    Trace(trace::Args),
}

fn main() -> ExitCode {
    let (name, done) = match Cli::parse().command {
        // Both print a line of their own after what they wrote.
        Command::Codegen => ("codegen", codegen::run().map(|summary| summary + "\n")),
        Command::Dump(args) => ("dump", dump::run(args).map(|summary| summary + "\n")),
        Command::Instructions(args) => ("instructions", instructions::run(args)),
        Command::Allocations(args) => ("allocations", allocations::run(args)),
        Command::Census(args) => ("census", census::run(args)),
        Command::Neow(args) => ("neow", neow::run(args)),
        Command::Trace(args) => ("trace", trace::run(args)),
    };
    match done {
        Ok(report) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{name} failed: {e}");
            ExitCode::FAILURE
        }
    }
}

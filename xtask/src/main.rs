//! Repo chores with real logic in them. `cargo xtask <command>`, and
//! `cargo xtask help` lists the commands.
//!
//! `codegen` turns the dumps in `sts2-content/src/<version>/dumps/` into that
//! crate's tables, one domain per dump. It fails the build if two dumps came
//! from different builds.
//!
//! The generated tree is committed rather than produced by a `build.rs`, so that
//! a content change reads as a diff. The whole point of this project is noticing
//! when the game and the simulator disagree. "Which 40 cards did that patch
//! touch" is worth being able to see.

mod allocations;
mod asm;
mod census;
mod codegen;
mod coverage;
mod dump;
mod error;
mod followed;
mod instructions;
mod neow;
mod plays;
mod record;
mod scenario_diff;
mod split;
mod trace;
mod unknown;

use std::path::{Path, PathBuf};
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
    Codegen(codegen::Args),
    /// Ask the game for its dumps.
    Dump(dump::Args),
    /// Count what a replayed step costs, in instructions.
    Instructions(instructions::Args),
    /// Name where a replayed step reaches the heap.
    Allocations(allocations::Args),
    /// Play runs at random and rank what stops them.
    Census(census::Args),
    /// Have the game play the census's runs, and keep them.
    Record(record::Args),
    /// Count how much of each act's content has code.
    Coverage(coverage::Args),
    /// Find the seeds whose Neow offers a relic.
    Neow(neow::Args),
    /// Show where a replayed step's instructions go, span by span.
    Trace(trace::Args),
    /// Find the seeds whose first unknown room rolls a given room.
    Unknown(unknown::Args),
    /// Show where a scenario's simulator and game part.
    ScenarioDiff(scenario_diff::Args),
    /// Write a recorded run, up to its first split, as a scenario.
    Split(split::Args),
}

/// The repository's root, which is where cargo and the profiles live.
pub(crate) fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits inside the workspace")
        .to_path_buf()
}

/// A command that failed, by the command.
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("codegen failed: {0}")]
    Codegen(#[from] codegen::Error),
    #[error("dump failed: {0}")]
    Dump(#[from] dump::Error),
    #[error("instructions failed: {0}")]
    Instructions(#[from] instructions::Error),
    #[error("allocations failed: {0}")]
    Allocations(#[from] allocations::Error),
    #[error("census failed: {0}")]
    Census(#[from] error::ProcessError),
    #[error("record failed: {0}")]
    Record(error::ProcessError),
    #[error("coverage failed: {0}")]
    Coverage(#[from] coverage::Error),
    #[error("neow failed: {0}")]
    Neow(#[from] neow::Error),
    #[error("trace failed: {0}")]
    Trace(#[from] trace::Error),
    #[error("unknown failed: {0}")]
    Unknown(error::ProcessError),
    #[error("scenario-diff failed: {0}")]
    ScenarioDiff(error::ProcessError),
    #[error("split failed: {0}")]
    Split(#[from] split::Error),
}

fn run(command: Command) -> Result<String, Error> {
    Ok(match command {
        // Both print a line of their own after what they wrote.
        Command::Codegen(args) => codegen::run(args)? + "\n",
        Command::Dump(args) => dump::run(args)? + "\n",
        Command::Instructions(args) => instructions::run(args)?,
        Command::Allocations(args) => allocations::run(args)?,
        Command::Census(args) => census::run(args)?,
        Command::Record(args) => record::run(args).map_err(Error::Record)?,
        Command::Coverage(args) => coverage::run(args)?,
        Command::Neow(args) => neow::run(args)?,
        Command::Trace(args) => trace::run(args)?,
        Command::Unknown(args) => unknown::run(args).map_err(Error::Unknown)?,
        Command::ScenarioDiff(args) => scenario_diff::run(args).map_err(Error::ScenarioDiff)?,
        Command::Split(args) => split::run(args)?,
    })
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(report) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

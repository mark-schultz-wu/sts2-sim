//! `cargo xtask game-trace <INDEX> <PATTERN>...`: the game's own calls in
//! the moves before a recorded run parts from the simulator.
//!
//! The action log says what each action did, and not the order of the
//! game's steps inside one. This plays the run on the game beside the
//! simulator, through `follow_run --diff`, with the mod's trace on. The
//! mod writes each call of a method a pattern names, with its arguments,
//! and each command it was sent. This prints the calls of the last
//! `--moves` moves before the split.
//!
//! A pattern is a type's name, short or full, or `Type.Method`, and a
//! trailing `*` matches any rest:
//!
//! ```text
//! cargo xtask game-trace 6 --file target/hunt/parted-v0.111.0.bin \
//!     'Hook.*' 'PowerCmd.*' DampenPower
//! ```
//!
//! The build is `STS2_GAME_VERSION`'s, as for `follow_run`. The whole
//! trace stays in `target/game-trace/<INDEX>.log`.

use std::path::PathBuf;
use std::process::Command;

use crate::error::{self, FileError, ProcessError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    Version(#[from] crate::codegen::VersionError),
    #[error("the game wrote no trace to {0}; is the deployed mod older than this one?")]
    NoTrace(PathBuf),
}

#[derive(clap::Args)]
pub struct Args {
    /// The run, by its place in the file, counted from zero.
    index: usize,
    /// The methods to trace.
    #[arg(required = true)]
    patterns: Vec<String>,
    /// The file of runs, such as the parted runs `cargo xtask hunt` keeps.
    /// The build's recorded file when none is named.
    #[arg(long)]
    file: Option<PathBuf>,
    /// How many moves before the split to print the calls of.
    #[arg(long, default_value_t = 2)]
    moves: usize,
}

pub fn run(
    Args {
        index,
        patterns,
        file,
        moves,
    }: Args,
) -> Result<String, Error> {
    let root = crate::workspace();
    let follow = error::build_example(&root, "follow_run")?;
    let file = match file {
        Some(file) => file,
        None => crate::followed::recorded_file(&root)?,
    };
    let dir = root.join("target/game-trace");
    error::create_dir(&dir)?;
    let log = dir.join(format!("{index}.log"));
    // A trace left from an earlier run would read as this one's.
    let _ = std::fs::remove_file(&log);
    // A run that parts exits with a failure, which is the point here.
    let followed = error::spawned(
        Command::new(&follow)
            .arg(&file)
            .arg(index.to_string())
            .arg("--diff")
            .arg(dir.join("diff"))
            .env("STS2_HEADLESS_TRACE", patterns.join(","))
            .env("STS2_HEADLESS_TRACE_FILE", &log),
    )?;
    if !log.exists() {
        return Err(Error::NoTrace(log));
    }
    let trace = error::read(&log)?;
    let mut report = last_moves(&trace, moves);
    report.push_str(String::from_utf8_lossy(&followed.stdout).trim_end());
    report.push('\n');
    Ok(report)
}

/// The calls from the `moves`-th move from the end on. A move is a command
/// that changes the run. An `observe` only reads it, and its calls belong to
/// the move before it.
fn last_moves(trace: &str, moves: usize) -> String {
    let lines: Vec<&str> = trace.lines().collect();
    let starts: Vec<usize> = (lines.iter().enumerate())
        .filter(|(_, line)| line.starts_with("> ") && !line.contains(r#""cmd":"observe""#))
        .map(|(at, _)| at)
        .collect();
    let from = starts.len().saturating_sub(moves);
    let first = starts.get(from).copied().unwrap_or(0);
    let mut report = String::new();
    for line in &lines[first..] {
        report.push_str(line);
        report.push('\n');
    }
    report
}

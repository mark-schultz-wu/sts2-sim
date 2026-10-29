//! `cargo xtask game-trace <INDEX> <PATTERN>...`: the game's own calls in
//! the moves before a recorded run parts from the simulator.
//!
//! The action log says what each action did, and not the order of the
//! game's steps inside one. This plays the run on the game beside the
//! simulator, through `sts2-run follow --diff`, with the mod's trace on. The
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
//! The build is `STS2_GAME_VERSION`'s, as for `sts2-run follow`. The whole
//! trace stays in `target/game-trace/<INDEX>.log`.
//!
//! With `--coop`, `INDEX` is a co-op seed's number, as `cargo xtask coop`
//! counts them. This plays that seed through `sts2-run --hunt`, one game
//! for each seat, and each game traces to a file of its own. It prints the
//! calls of each seat's game over the last `--moves` moves of the run, and
//! the whole of each trace stays in `target/game-trace/<SEED>/`:
//!
//! ```text
//! STS2_GAME_VERSION=v0.111.0 cargo xtask game-trace --coop 11 FrailPower
//! ```

use std::fmt::Write as _;
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
    /// The run, by its place in the file, counted from zero. With
    /// `--coop`, the co-op seed's number.
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
    /// Play the co-op seed `INDEX` names, one game for each seat.
    #[arg(long, conflicts_with = "file")]
    coop: bool,
    /// Each co-op seat's character, by id, in seat order.
    #[arg(long, value_delimiter = ',', default_value = "IRONCLAD,SILENT")]
    characters: Vec<String>,
}

pub fn run(
    Args {
        index,
        patterns,
        file,
        moves,
        coop,
        characters,
    }: Args,
) -> Result<String, Error> {
    if coop {
        return coop_trace(index, &patterns, moves, &characters);
    }
    let root = crate::workspace();
    let follow = error::build_runner(&root)?;
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
            .arg("follow")
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

/// The co-op seed `seed` played with the trace on, and each seat's calls over
/// the last `moves` moves.
///
/// A seat's trace holds a command for each move its own player made, in the
/// order the run's log lists them. The other players' moves reach its game
/// over the network, with no command. So each seat's calls are cut at its
/// own last move before the window, which leads into it.
fn coop_trace(
    seed: usize,
    patterns: &[String],
    moves: usize,
    characters: &[String],
) -> Result<String, Error> {
    let root = crate::workspace();
    let runner = error::build_runner(&root)?;
    let seed = crate::runner::seed(seed);
    let homes = root.join("target/game-trace").join(&seed);
    // A trace left from an earlier run would read as this one's.
    let _ = std::fs::remove_dir_all(&homes);
    error::create_dir(&homes)?;
    let played = error::spawned(
        Command::new(&runner)
            .args(["play", seed.as_str(), &characters.join(",")])
            .args(["--build", &crate::runner::game_build()])
            .args(["--table", "game", "--carried", "--moves", "--hunt"])
            .arg("--home")
            .arg(&homes)
            .arg("--record")
            .arg(homes.join("run.bin"))
            .env("STS2_HEADLESS_TRACE", patterns.join(",")),
    )?;
    let log = String::from_utf8_lossy(&played.stdout);
    error::write(&homes.join("moves.log"), &played.stdout)?;
    // `<step> seat <seat>: <move>`, for each move.
    let movers: Vec<usize> = (log.lines())
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            words.next()?.parse::<usize>().ok()?;
            (words.next()? == "seat").then_some(())?;
            words.next()?.trim_end_matches(':').parse().ok()
        })
        .collect();
    let window = &movers[movers.len().saturating_sub(moves)..];
    let mut report = String::new();
    for seat in 0..characters.len() {
        let file = homes.join(format!("seat{seat}.trace"));
        if !file.exists() {
            return Err(Error::NoTrace(file));
        }
        let trace = error::read(&file)?;
        let own = window.iter().filter(|&&mover| mover == seat).count();
        writeln!(report, "== seat {seat}").expect("a String takes a line");
        report.push_str(&last_moves(&trace, own + 1));
    }
    writeln!(report, "{}", log.lines().last().unwrap_or("no word why"))
        .expect("a String takes a line");
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

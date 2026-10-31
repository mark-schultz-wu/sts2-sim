//! The game plays co-op runs at random, one game for each seat.
//!
//! Each run is `sts2-run` on the game for one seed. There is one game
//! process for each seat over ENet, and every seat picks its moves at random
//! from its own legal list. A run that ends is fine. A refused move, an error a game
//! logged, or seats that stall are findings, and are named.
//!
//! The runs that end go into `target/coop/runs.bin`, one file of recorded
//! runs, as a hunt keeps its own.
//!
//! ```text
//! STS2_GAME_VERSION=v0.111.0 cargo xtask coop
//! STS2_GAME_VERSION=v0.111.0 cargo xtask coop --runs 20 --characters IRONCLAD,SILENT,DEFECT
//! ```
//!
//! The build is `STS2_GAME_VERSION`'s. Only v0.111.0 plays co-op.
//!
//! With `--hunt`, the simulator plays beside every seat, and each run stops
//! where the two part. The report counts the runs that agreed, parted, or
//! where the simulator panicked, and how many steps they matched in all.

use std::path::Path;
use std::process::Command;

use sts2_core::End;

#[derive(clap::Args)]
pub struct Args {
    /// Seeds to play.
    #[arg(long, default_value_t = 6)]
    runs: usize,
    /// The first seed's number. Seed `n` is `C` and `n` in nine digits.
    #[arg(long, default_value_t = 0)]
    first_seed: usize,
    /// Each seat's character, by id, in seat order.
    #[arg(long, value_delimiter = ',', default_value = "IRONCLAD,SILENT")]
    characters: Vec<String>,
    /// How many runs play at once.
    #[arg(long, default_value_t = 4)]
    width: usize,
    /// Play the simulator beside the game, and stop each run where they part.
    #[arg(long)]
    hunt: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Process(#[from] crate::error::ProcessError),
    #[error(transparent)]
    File(#[from] crate::error::FileError),
}

pub fn run(args: Args) -> Result<String, Error> {
    let Args {
        runs,
        first_seed,
        characters,
        width,
        hunt,
    } = args;
    let root = crate::workspace();
    let runner = crate::error::build_runner(&root)?;
    let build = crate::runner::game_build();
    let scratch = root.join("target/coop");
    let _ = std::fs::remove_dir_all(&scratch);
    crate::error::create_dir(&scratch)?;

    let seeds: Vec<String> = (first_seed..first_seed + runs)
        .map(crate::runner::seed)
        .collect();
    let characters = characters.join(",");
    let went: Vec<(usize, Result<String, String>)> =
        crate::runner::pool(&seeds, width, |_, at, seed| {
            (
                at,
                play_one(&runner, &build, seed, &characters, &scratch, hunt),
            )
        });

    let ended = went.iter().filter(|(_, went)| went.is_ok()).count();
    let kept: Vec<_> = went
        .iter()
        .filter(|(_, went)| went.is_ok())
        .map(|(at, _)| run_file(&scratch, &seeds[*at]))
        // A hunt that stopped before the first move kept no run.
        .filter(|file| file.exists())
        .collect();
    let file = scratch.join("runs.bin");
    if !kept.is_empty() {
        crate::error::status(
            Command::new(crate::error::merger(&root)?)
                .arg(&file)
                .args(&kept),
        )?;
    }
    let mut out = String::new();
    for (at, went) in &went {
        let line = match went {
            Ok(ending) => ending.clone(),
            Err(why) => format!("FAILED {why}"),
        };
        out += &format!("{} {characters}: {line}\n", seeds[*at]);
    }
    out += &format!("{ended} of {} runs ended\n", went.len());
    if hunt {
        out += &verdicts(went.iter().filter_map(|(_, went)| went.as_deref().ok()));
    }
    if !kept.is_empty() {
        out += &format!("the runs that ended are in {}\n", file.display());
    }
    Ok(out)
}

/// How the hunted runs that ended came out: how many of each verdict, and
/// the steps the simulator matched across them. Each runner's last line
/// opens with its verdict and the step it reached.
fn verdicts<'a>(lines: impl Iterator<Item = &'a str>) -> String {
    let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
    let mut matched = 0;
    for line in lines {
        let mut words = line.split_whitespace();
        let verdict = words.next().unwrap_or("nothing");
        *counts.entry(verdict).or_default() += 1;
        matched += words
            .next()
            .and_then(|step| step.trim_end_matches([',', ':']).parse::<usize>().ok())
            .unwrap_or(0);
    }
    let counted: Vec<String> = counts
        .iter()
        .map(|(verdict, n)| format!("{n} {verdict}"))
        .collect();
    format!("hunted: {}; {matched} steps matched\n", counted.join(", "))
}

/// Where the run of `seed` is written.
fn run_file(scratch: &Path, seed: &str) -> std::path::PathBuf {
    scratch.join(seed).join("run.bin")
}

/// Plays one run, and says how it ended or why it did not. That is its
/// verdict, as `<end> <step> seat <seat>: <detail>`. A run that stopped
/// short names the move it came after.
///
/// Each move goes to a log beside the seats' profiles, which the failure
/// names.
fn play_one(
    runner: &Path,
    build: &str,
    seed: &str,
    characters: &str,
    scratch: &Path,
    hunt: bool,
) -> Result<String, String> {
    let homes = scratch.join(seed);
    let mut args = crate::runner::args(["play", seed, characters, "--build", build]);
    args.extend(crate::runner::args([
        "--table",
        "game",
        "--carried",
        "--moves",
    ]));
    args.extend(crate::runner::args([
        "--record".as_ref(),
        run_file(scratch, seed).as_os_str(),
    ]));
    args.extend(crate::runner::args(["--home".as_ref(), homes.as_os_str()]));
    if hunt {
        args.extend(crate::runner::args(["--hunt"]));
    }
    let ran = crate::runner::once(runner, &args, &[]);
    let log = homes.join("moves.log");
    let _ = std::fs::write(&log, &ran.stdout);
    let verdict = ran
        .verdict
        .map_err(|why| format!("{why} (moves in {})", log.display()))?;
    let seat = verdict
        .seat
        .map(|seat| format!(" seat {}", seat.0))
        .unwrap_or_default();
    // `<step> seat <seat>: <move>`, the last one played.
    let last = ran.stdout.lines().rev().find_map(|line| {
        let (_, rest) = line.split_once(" seat ")?;
        let (seat, played) = rest.split_once(": ")?;
        Some(format!(", after seat {seat}'s {played}"))
    });
    let line = |after: Option<String>| {
        format!(
            "{} {}{seat}: {}{}",
            verdict.end,
            verdict.step,
            verdict.detail,
            after.unwrap_or_default()
        )
    };
    match verdict.end {
        // Every machine's game logs the shared actions in the order it took
        // them in, and every machine follows the host's.
        End::Parted => Ok(line(Some(format!(
            " (the host's order in {})",
            homes.join("seat0/SlayTheSpire2/order.jsonl").display()
        )))),
        End::Won | End::Lost | End::Capped | End::Agreed => Ok(line(None)),
        End::Panicked | End::Refused => Ok(line(last)),
        _ => Err(format!("{} (moves in {})", line(last), log.display())),
    }
}

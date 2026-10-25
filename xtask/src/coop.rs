//! The game plays co-op runs at random, one game for each seat.
//!
//! Each run is `sts2-golden`'s `coop_run` for one seed. There is one game
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
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    let runner = crate::error::build_example(&root, "coop_run")?;
    let merger = crate::error::build_example(&root, "merge_runs")?;
    let scratch = root.join("target/coop");
    let _ = std::fs::remove_dir_all(&scratch);
    crate::error::create_dir(&scratch)?;

    let seeds: Vec<String> = (first_seed..first_seed + runs)
        .map(|at| format!("C{at:09}"))
        .collect();
    let characters = characters.join(",");
    let next = AtomicUsize::new(0);
    let went = Mutex::new(Vec::with_capacity(seeds.len()));
    std::thread::scope(|scope| {
        for _ in 0..width.max(1) {
            let (seeds, next, went, runner, scratch, characters) =
                (&seeds, &next, &went, &runner, &scratch, &characters);
            scope.spawn(move || {
                while let Some(at) =
                    Some(next.fetch_add(1, Ordering::Relaxed)).filter(|&at| at < seeds.len())
                {
                    let result = play_one(runner, &seeds[at], characters, scratch, hunt);
                    went.lock().expect("no thread panics").push((at, result));
                }
            });
        }
    });
    let mut went = went.into_inner().expect("no thread panics");
    went.sort_by_key(|(at, _)| *at);

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
        crate::error::status(Command::new(&merger).arg(&file).args(&kept))?;
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
            .and_then(|step| step.trim_end_matches(',').parse::<usize>().ok())
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

/// Plays one run, and says how it ended or why it did not.
///
/// Each move goes to a log beside the seats' profiles, which the failure
/// names.
fn play_one(
    runner: &Path,
    seed: &str,
    characters: &str,
    scratch: &Path,
    hunt: bool,
) -> Result<String, String> {
    let homes = scratch.join(seed);
    let output = Command::new(runner)
        .args([seed, characters])
        .arg("--homes")
        .arg(&homes)
        .arg("--out")
        .arg(run_file(scratch, seed))
        .args(hunt.then_some("--hunt"))
        .output()
        .map_err(|e| format!("launching the runner: {e}"))?;
    let log = homes.join("moves.log");
    let _ = std::fs::write(&log, &output.stdout);
    let last = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .lines()
            .last()
            .unwrap_or("no word why")
            .to_owned()
    };
    match output.status.success() {
        true => Ok(last(&output.stdout)),
        false => Err(format!(
            "{} (moves in {})",
            last(&output.stderr),
            log.display()
        )),
    }
}

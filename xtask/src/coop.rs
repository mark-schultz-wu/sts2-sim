//! The game plays co-op runs at random, one game for each seat.
//!
//! Each run is `sts2-golden`'s `coop_run` for one seed. There is one game
//! process for each seat over ENet, and every seat picks its moves at random
//! from its own legal list. A run that ends is fine. A refused move, an error a game
//! logged, or seats that stall are findings, and are named.
//!
//! ```text
//! STS2_GAME_VERSION=v0.111.0 cargo xtask coop
//! STS2_GAME_VERSION=v0.111.0 cargo xtask coop --runs 20 --characters IRONCLAD,SILENT,DEFECT
//! ```
//!
//! The build is `STS2_GAME_VERSION`'s. Only v0.111.0 plays co-op.

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
    } = args;
    let root = crate::workspace();
    let runner = crate::error::build_example(&root, "coop_run")?;
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
                    let result = play_one(runner, &seeds[at], characters, scratch);
                    went.lock().expect("no thread panics").push((at, result));
                }
            });
        }
    });
    let mut went = went.into_inner().expect("no thread panics");
    went.sort_by_key(|(at, _)| *at);

    let ended = went.iter().filter(|(_, went)| went.is_ok()).count();
    let mut out = String::new();
    for (at, went) in &went {
        let line = match went {
            Ok(ending) => ending.clone(),
            Err(why) => format!("FAILED {why}"),
        };
        out += &format!("{} {characters}: {line}\n", seeds[*at]);
    }
    out += &format!("{ended} of {} runs ended\n", went.len());
    Ok(out)
}

/// Plays one run, and says how it ended or why it did not.
///
/// Each move goes to a log beside the seats' profiles, which the failure
/// names.
fn play_one(runner: &Path, seed: &str, characters: &str, scratch: &Path) -> Result<String, String> {
    let homes = scratch.join(seed);
    let output = Command::new(runner)
        .args([seed, characters])
        .arg("--homes")
        .arg(&homes)
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

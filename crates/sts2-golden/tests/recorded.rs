//! The committed file of runs the game played, read back and followed.
//!
//! `cargo xtask record` writes it, on purpose and never in CI. This holds it
//! to what the recorder plays: every census seed for every character, each
//! run whole. Then it follows each run on the simulator, and holds the
//! positions that agree to `recorded.matched`. That count fails the test if
//! it falls, and asks to be raised if it rises, the way `BASELINE` does.
//! `cargo xtask coverage` says where the runs part.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct as _, Gift, Play as _, Setup};
use sts2_golden::recorded::{Followed, Recorded, Run, follow};
use sts2_sim::Sim;

#[path = "../examples/carried/mod.rs"]
mod carried;
use carried::CARRIED;

type V = V0_107_1;

/// The seeds a character plays, as `cargo xtask record` plays them.
const RUNS: usize = 300;

fn here(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(file)
}

fn recorded() -> Recorded {
    let path = here("recorded.bin");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    Recorded::decode(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The census characters, which every ascension's runs play alike.
const CHARACTERS: [&str; 5] = ["DEFECT", "IRONCLAD", "NECROBINDER", "REGENT", "SILENT"];

#[test]
fn the_committed_file_holds_every_census_run() {
    let recorded = recorded();
    // Each ascension's seeds, by character.
    let mut seeds: BTreeMap<&str, BTreeMap<&str, Vec<&str>>> = BTreeMap::new();
    for run in &recorded.runs {
        seeds
            .entry(&run.ascension)
            .or_default()
            .entry(&run.character)
            .or_default()
            .push(&run.seed);
    }
    let first =
        |count: usize| -> Vec<String> { (0..count).map(|at| format!("C{at:09}")).collect() };
    for (ascension, mut by_character) in seeds {
        // The census itself plays at no ascension. A rung's runs play the
        // census's first seeds, as many for every character.
        let want = match ascension {
            "None" => first(RUNS),
            _ => first(by_character.get("IRONCLAD").map_or(0, Vec::len)),
        };
        for character in CHARACTERS {
            let mut played = by_character.remove(character).unwrap_or_default();
            played.sort_unstable();
            assert_eq!(played, want, "{character}'s seeds at {ascension}");
        }
        assert!(
            by_character.is_empty(),
            "runs at {ascension} for no census character: {by_character:?}"
        );
    }
}

/// The positions of `run` the simulator agrees with, from the start.
fn matched(run: &Run) -> usize {
    let mut sim = Sim::<V>::begin((), &run.start()).expect("the run starts");
    for (relic, count) in CARRIED {
        for _ in 0..count {
            sim.setup(&Setup::Grant(Gift::Relic(relic)))
                .expect("a grant applies")
                .expect("and is not refused");
        }
    }
    match follow(run, &mut sim) {
        Followed::Whole => run.digests.len(),
        Followed::PartedAt { step, .. } => step,
    }
}

#[test]
fn the_simulator_follows_the_recorded_runs_no_less_far() {
    let recorded = recorded();
    let path = here("recorded.matched");
    let floor: usize = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("{} is a step count: {e}", path.display()));

    // A run takes a second or so unoptimised, so they are shared out.
    let next = AtomicUsize::new(0);
    let total = AtomicUsize::new(0);
    let failed = Mutex::new(None);
    let width = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|scope| {
        for _ in 0..width {
            scope.spawn(|| {
                while let Some(run) = recorded.runs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    // A panic names the run, then fails the test once every
                    // other run is done.
                    let followed = std::thread::scope(|inner| inner.spawn(|| matched(run)).join());
                    match followed {
                        Ok(steps) => {
                            total.fetch_add(steps, Ordering::Relaxed);
                        }
                        Err(_) => {
                            *failed.lock().expect("no panic holds it") =
                                Some(format!("{} {}", run.character, run.seed));
                        }
                    }
                }
            });
        }
    });
    if let Some(run) = failed.into_inner().expect("no panic holds it") {
        panic!("the simulator panicked following {run}, a move the game played");
    }

    let total = total.into_inner();
    let all: usize = recorded.runs.iter().map(|run| run.digests.len()).sum();
    assert!(
        total >= floor,
        "the simulator follows {total} of {all} recorded positions, down from {floor}. \
         `cargo xtask coverage` says where the runs part."
    );
    assert!(
        total <= floor,
        "the simulator follows {total} of {all} recorded positions, up from {floor}. \
         Raise {} to {total} so the next change is held to it.",
        path.display(),
    );
}

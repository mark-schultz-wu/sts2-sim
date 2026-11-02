//! The committed files of runs the game played, read back and followed.
//!
//! `cargo xtask record` writes them, one per build, on purpose and never in
//! CI. This holds each to what the recorder plays: every census seed for
//! every character, each run whole, on its own build. Then it follows each
//! build's runs on the simulator, and holds the positions that agree to a
//! count of the build's own. That is `recorded.matched` for v0.107.1's, and
//! `recorded-v0.111.0.matched` for v0.111.0's. A count fails the test if
//! it falls, and asks to be raised if it rises, the way `BASELINE` does.
//! `cargo xtask coverage` says where the runs part.
//!
//! `hunted.bin`, `hunted-v0.111.0.bin` and the co-op `coop-v0.111.0.bin`
//! hold hunted runs, recorded whole once fixed. Each is followed against
//! its own `.matched`, and holds no census.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use sts2_content::v0_107_1::V0_107_1;
use sts2_content::v0_111_0::V0_111_0;
use sts2_core::RunTable as _;
use sts2_golden::recorded::{Ending, Followed, Recorded, Run, follow};
use sts2_run::Carried;
use sts2_sim::SimTable;

type V = V0_107_1;

/// The seeds a character plays, as `cargo xtask record` plays them.
const RUNS: usize = 300;

fn here(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(file)
}

fn recorded(file: &str) -> Recorded {
    let path = here(file);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    Recorded::decode(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The census characters, which every ascension's runs play alike.
const CHARACTERS: [&str; 5] = ["DEFECT", "IRONCLAD", "NECROBINDER", "REGENT", "SILENT"];

/// What a run was started with besides its seed and character: its
/// ascension and its modifiers.
type RunSetup<'a> = (&'a str, &'a [String]);

#[test]
fn the_committed_file_holds_every_census_run() {
    holds_every_census_run(&recorded("recorded.bin"), &sts2_content::v0_107_1::VERSION);
}

#[test]
fn the_committed_v0_111_0_file_holds_every_census_run() {
    holds_every_census_run(
        &recorded("recorded-v0.111.0.bin"),
        &sts2_content::v0_111_0::VERSION,
    );
}

/// Every census seed for every character, recorded on `build`.
fn holds_every_census_run(recorded: &Recorded, build: &sts2_core::Version) {
    assert_eq!(
        recorded.stamp.assembly, build.assembly_hash,
        "recorded on {}",
        build.name
    );
    // Each setup's seeds, by character: its ascension and its modifiers.
    let mut seeds: BTreeMap<RunSetup<'_>, BTreeMap<String, Vec<&str>>> = BTreeMap::new();
    for run in &recorded.runs {
        seeds
            .entry((&run.ascension, &run.modifiers))
            .or_default()
            .entry(run.characters())
            .or_default()
            .push(&run.seed);
    }
    let first =
        |count: usize| -> Vec<String> { (0..count).map(|at| format!("C{at:09}")).collect() };
    for (setup, mut by_character) in seeds {
        // The census itself plays at no ascension and with no modifiers.
        // Any other setup's runs play the census's first seeds, as many for
        // every character.
        let want = match setup {
            ("None", []) => first(RUNS),
            _ => first(by_character.get("IRONCLAD").map_or(0, Vec::len)),
        };
        for character in CHARACTERS {
            let mut played = by_character.remove(character).unwrap_or_default();
            played.sort_unstable();
            assert_eq!(played, want, "{character}'s seeds at {setup:?}");
        }
        assert!(
            by_character.is_empty(),
            "runs at {setup:?} for no census character: {by_character:?}"
        );
    }
}

/// The positions of `run` the simulator agrees with, from the start, as
/// [`follow`] compares them.
///
/// A hunt stops a run on the move a simulator panicked on, and the run ends
/// there, parted. That move is a known gap. A panic on any move before it is
/// not.
fn matched<V: sts2_content::Build>(run: &Run) -> usize {
    let agreed = AtomicUsize::new(0);
    let followed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut table =
            Carried::<SimTable<V>>::begin((), &run.start()).expect("the run starts and carries");
        follow(run, &mut table, true, |count| {
            agreed.store(count, Ordering::Relaxed)
        })
        .unwrap_or_else(|fault| panic!("following the run: {fault}"))
    }));
    match followed {
        Ok(Followed::Whole) => run.digests.len(),
        Ok(Followed::PartedAt { step, .. }) => step,
        Err(_)
            if run.ending == Ending::Parted
                && agreed.load(Ordering::Relaxed) == run.moves.len() =>
        {
            run.moves.len()
        }
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

#[test]
fn the_simulator_follows_the_recorded_runs_no_less_far() {
    follows_no_less_far("recorded.bin", "recorded.matched", matched::<V>);
}

#[test]
fn the_simulator_follows_the_hunted_runs_no_less_far() {
    follows_no_less_far("hunted.bin", "hunted.matched", matched::<V>);
}

#[test]
fn the_simulator_follows_the_v0_111_0_hunted_runs_no_less_far() {
    follows_no_less_far(
        "hunted-v0.111.0.bin",
        "hunted-v0.111.0.matched",
        matched::<V0_111_0>,
    );
}

#[test]
fn the_simulator_follows_the_v0_111_0_runs_no_less_far() {
    follows_no_less_far(
        "recorded-v0.111.0.bin",
        "recorded-v0.111.0.matched",
        matched::<V0_111_0>,
    );
}

#[test]
fn the_simulator_follows_the_coop_runs_no_less_far() {
    follows_no_less_far(
        "coop-v0.111.0.bin",
        "coop-v0.111.0.matched",
        matched::<V0_111_0>,
    );
}

/// Follows every run of `file` with `matched`, and holds the positions that
/// agree to the count in `count`.
fn follows_no_less_far(file: &str, count: &str, matched: fn(&Run) -> usize) {
    let recorded = recorded(file);
    let path = here(count);
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
                                Some(format!("{} {}", run.characters(), run.seed));
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
         `cargo xtask coverage` says where the runs part, and \
         `cargo xtask coop --hunt` where a co-op run does."
    );
    assert!(
        total <= floor,
        "the simulator follows {total} of {all} recorded positions, up from {floor}. \
         Raise {} to {total} so the next change is held to it.",
        path.display(),
    );
}

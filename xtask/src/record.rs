//! The game plays the census's runs, and they are kept.
//!
//! Each run is `sts2-golden`'s `record_run` for one seed and character: the
//! game playing at random from its own legal moves, carrying what the census
//! carries. The runs go into one file, which the simulator is held to.
//!
//! ```text
//! cargo xtask record
//! cargo xtask record --runs 10 --character SILENT --width 8
//! cargo xtask record --runs 50 --ascension DoubleBoss --append
//! STS2_GAME_VERSION=v0.111.0 cargo xtask record
//! cargo xtask record --again crates/sts2-golden/tests/hunted.bin
//! ```
//!
//! The build is `STS2_GAME_VERSION`'s, and its runs go to its own file, which
//! `cargo xtask coverage` reads under the same setting.
//!
//! A game waits on its frames rather than the processor, so many run at once,
//! each with a profile of its own. One sometimes crashes, and the run is
//! played again. A run that fails any other way is a finding, and is named.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The characters played when none is named, which are the census's.
const CHARACTERS: [&str; 5] = ["IRONCLAD", "SILENT", "REGENT", "NECROBINDER", "DEFECT"];

/// How many times a run whose game crashed is played before it counts as
/// failed.
const TRIES: usize = 3;

/// The exit `record_run` gives when the game stopped answering.
const CRASHED: i32 = 3;

#[derive(clap::Args)]
pub struct Args {
    /// Seeds to play for each character.
    #[arg(long, default_value_t = 300)]
    runs: usize,
    /// A character to play, by id. Every playable one when none is named.
    #[arg(long = "character")]
    characters: Vec<String>,
    /// How many games play at once.
    #[arg(long, default_value_t = 32)]
    width: usize,
    /// Where the runs go. The build's own recorded file when none is named.
    #[arg(long)]
    out: Option<PathBuf>,
    /// The ascension level every run plays at, by the id the game spells it
    /// with.
    #[arg(long, default_value = "None")]
    ascension: String,
    /// The modifiers every run starts with, by the ids the game spells them
    /// with, such as `FLIGHT`.
    #[arg(long, value_delimiter = ',')]
    modifiers: Vec<String>,
    /// Add the runs to those `out` already holds, rather than replace them.
    /// Both have to come from one build of the game.
    #[arg(long)]
    append: bool,
    /// Play the runs this file holds again, rather than new seeds.
    ///
    /// Each run keeps its seed, character, ascension and modifiers, and the
    /// runs go back in their order, to this file unless `--out` names
    /// another. A run's moves are the game's random picks from its seed, so
    /// it picks the same moves while the lists are the same. This records a
    /// new kind of move into the runs already kept.
    #[arg(long, value_name = "FILE", conflicts_with_all = ["runs", "characters", "ascension", "modifiers", "append"])]
    again: Option<PathBuf>,
}

/// One run that did not come back.
struct Failed {
    seed: String,
    character: String,
    why: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Process(#[from] crate::error::ProcessError),
    #[error(transparent)]
    Version(#[from] crate::codegen::VersionError),
}

pub fn run(args: Args) -> Result<String, Error> {
    let Args {
        runs,
        mut characters,
        width,
        out,
        ascension,
        modifiers,
        append,
        again,
    } = args;
    if characters.is_empty() {
        characters = CHARACTERS.iter().map(|c| c.to_string()).collect();
    }
    let root = crate::workspace();
    let out = match (out, &again) {
        (Some(out), _) => out,
        (None, Some(again)) => again.clone(),
        (None, None) => crate::followed::recorded_file(&root)?,
    };
    let recorder = crate::error::build_example(&root, "record_run")?;
    let merger = crate::error::build_example(&root, "merge_runs")?;

    // Each run to play: its seed, its character and the runner's setup flags.
    let jobs: Vec<(String, String, Vec<String>)> = match &again {
        Some(file) => listed(&root, file)?,
        None => {
            let setup = setup(ascension, &modifiers);
            characters
                .iter()
                .flat_map(|character| {
                    (0..runs).map({
                        let setup = setup.clone();
                        move |at| (format!("C{at:09}"), character.clone(), setup.clone())
                    })
                })
                .collect()
        }
    };

    let scratch = root.join("target/record");
    // Rewritten from nothing, so a run kept from another build of the mod
    // cannot join this one.
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(scratch.join("runs")).map_err(|source| {
        crate::error::ProcessError::Io {
            what: "making the scratch directory",
            source,
        }
    })?;

    let next = AtomicUsize::new(0);
    let retried = AtomicUsize::new(0);
    let failed = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for worker in 0..width.max(1) {
            let home = scratch.join(format!("home{worker}"));
            let (jobs, next, retried, failed, recorder, scratch) =
                (&jobs, &next, &retried, &failed, &recorder, &scratch);
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some((seed, character, setup)) = jobs.get(index) else {
                        break;
                    };
                    let at = run_file(scratch, index, character, seed);
                    let run = (seed.as_str(), character.as_str(), setup.as_slice());
                    if let Err(why) = record_one(recorder, run, &at, &home, retried) {
                        failed.lock().expect("no thread panics").push(Failed {
                            seed: seed.clone(),
                            character: character.clone(),
                            why,
                        });
                    }
                }
            });
        }
    });

    let failed = failed.into_inner().expect("no thread panics");
    let kept: Vec<PathBuf> = jobs
        .iter()
        .enumerate()
        .map(|(index, (seed, character, _))| run_file(&scratch, index, character, seed))
        .filter(|file| file.is_file())
        .collect();
    let target = root.join(&out);
    // The runs `out` held go first, when these join them.
    let held = (append && target.is_file()).then(|| {
        let held = scratch.join("held.bin");
        std::fs::copy(&target, &held).map(|_| held)
    });
    let held = held
        .transpose()
        .map_err(|source| crate::error::ProcessError::Io {
            what: "keeping the runs already recorded",
            source,
        })?;
    crate::error::status(Command::new(&merger).arg(&target).args(&held).args(&kept))?;

    let mut report = format!(
        "{} of {} runs recorded into {}, {} played again after a crash\n",
        kept.len(),
        jobs.len(),
        out.display(),
        retried.load(Ordering::Relaxed),
    );
    for Failed {
        seed,
        character,
        why,
    } in &failed
    {
        report.push_str(&format!("  failed {seed} {character}: {why}\n"));
    }
    Ok(report)
}

/// Where one run is written before the runs are merged. The index keeps two
/// runs of one seed and character apart, as a file played `--again` can
/// hold at two setups.
fn run_file(scratch: &Path, index: usize, character: &str, seed: &str) -> PathBuf {
    scratch
        .join("runs")
        .join(format!("{index:06}-{character}-{seed}.bin"))
}

/// The runs a file holds, as `list_runs` lists them, each with its setup.
fn listed(root: &Path, file: &Path) -> Result<Vec<(String, String, Vec<String>)>, Error> {
    let lister = crate::error::build_example(root, "list_runs")?;
    let listed = crate::error::output(Command::new(&lister).arg(root.join(file)))?;
    Ok(String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let (seed, character, ascension, modifiers) =
                (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
            let modifiers: Vec<String> = match modifiers {
                "-" => Vec::new(),
                listed => listed.split(',').map(str::to_owned).collect(),
            };
            Some((
                seed.to_owned(),
                character.to_owned(),
                setup(ascension.to_owned(), &modifiers),
            ))
        })
        .collect())
}

/// The runner's flags for a run at `ascension` with `modifiers`, which
/// `record_run` and `random_run` both read.
pub(crate) fn setup(ascension: String, modifiers: &[String]) -> Vec<String> {
    let mut setup = vec!["--ascension".to_owned(), ascension];
    if !modifiers.is_empty() {
        setup.extend(["--modifiers".to_owned(), modifiers.join(",")]);
    }
    setup
}

/// Plays one run, again after a crash, and says why it failed if it did.
fn record_one(
    recorder: &Path,
    (seed, character, setup): (&str, &str, &[String]),
    out: &Path,
    home: &Path,
    retried: &AtomicUsize,
) -> Result<(), String> {
    for attempt in 0..TRIES {
        if attempt > 0 {
            retried.fetch_add(1, Ordering::Relaxed);
        }
        let output = Command::new(recorder)
            .args([seed, character])
            .arg(out)
            .args(setup)
            .arg("--data-home")
            .arg(home)
            .output()
            .map_err(|e| format!("launching the recorder: {e}"))?;
        match output.status.code() {
            Some(0) => return Ok(()),
            Some(CRASHED) => continue,
            _ => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(stderr.lines().last().unwrap_or("no word why").to_owned());
            }
        }
    }
    Err(format!("the game crashed {TRIES} times"))
}

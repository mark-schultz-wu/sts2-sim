//! The game plays the census's runs, and they are kept.
//!
//! Each run is `sts2-golden`'s `record_run` for one seed and character: the
//! game playing at random from its own legal moves, carrying what the census
//! carries. The runs go into one file, which the simulator is held to.
//!
//! ```text
//! cargo xtask record
//! cargo xtask record --runs 10 --character SILENT --width 8
//! ```
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
    /// Where the runs go.
    #[arg(long, default_value = "crates/sts2-golden/tests/recorded.bin")]
    out: PathBuf,
}

/// One run that did not come back.
struct Failed {
    seed: String,
    character: String,
    why: String,
}

pub fn run(args: Args) -> Result<String, crate::error::ProcessError> {
    let Args {
        runs,
        mut characters,
        width,
        out,
    } = args;
    if characters.is_empty() {
        characters = CHARACTERS.iter().map(|c| c.to_string()).collect();
    }
    let root = crate::workspace();
    let recorder = crate::error::build_example(&root, "record_run")?;
    let merger = crate::error::build_example(&root, "merge_runs")?;

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

    let jobs: Vec<(String, String)> = characters
        .iter()
        .flat_map(|character| (0..runs).map(move |at| (format!("C{at:09}"), character.clone())))
        .collect();
    let next = AtomicUsize::new(0);
    let retried = AtomicUsize::new(0);
    let failed = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for worker in 0..width.max(1) {
            let home = scratch.join(format!("home{worker}"));
            let (jobs, next, retried, failed, recorder, scratch) =
                (&jobs, &next, &retried, &failed, &recorder, &scratch);
            scope.spawn(move || {
                while let Some((seed, character)) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let at = run_file(scratch, character, seed);
                    if let Err(why) = record_one(recorder, seed, character, &at, &home, retried) {
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
        .map(|(seed, character)| run_file(&scratch, character, seed))
        .filter(|file| file.is_file())
        .collect();
    let target = root.join(&out);
    crate::error::status(Command::new(&merger).arg(&target).args(&kept))?;

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

/// Where one run is written before the runs are merged.
fn run_file(scratch: &Path, character: &str, seed: &str) -> PathBuf {
    scratch.join("runs").join(format!("{character}-{seed}.bin"))
}

/// Plays one run, again after a crash, and says why it failed if it did.
fn record_one(
    recorder: &Path,
    seed: &str,
    character: &str,
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

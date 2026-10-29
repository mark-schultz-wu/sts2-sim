//! The game and the simulator play runs side by side, through `sts2-run
//! --hunt`. A run the two agree on is counted and thrown away. A
//! run they part on is kept up to the game's position where they first differ.
//!
//! ```text
//! cargo xtask hunt --runs 100 --first-seed 1000
//! cargo xtask hunt --runs 100 --first-seed 1000 --mixed
//! STS2_GAME_VERSION=v0.111.0 cargo xtask hunt --runs 20 --ascension DoubleBoss
//! ```
//!
//! The build is `STS2_GAME_VERSION`'s, and its parted runs go to
//! `target/hunt/parted-<build>.bin`. `follow_run` reads that file as it reads
//! a recorded one, and `cargo xtask split --file` writes a run of it as a
//! scenario.
//!
//! With none parted, `n` agreed runs put the rate at which a run parts below
//! 3 in `n`, at 95% confidence.
//!
//! `--coverage` has the game count which of its rules methods each run
//! enters, into `target/game-coverage/<build>/`, one file for each run.
//! `cargo xtask game-coverage` adds them up.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use sts2_core::End;

/// The characters played when none is named, which are the census's.
const CHARACTERS: [&str; 5] = ["IRONCLAD", "SILENT", "REGENT", "NECROBINDER", "DEFECT"];

/// The ascension levels above none, by the ids the game spells them with.
const ASCENSIONS: [&str; 10] = [
    "WearyTraveler",
    "AscendersBane",
    "Inflation",
    "Scarcity",
    "TightBelt",
    "Poverty",
    "ToughEnemies",
    "DeadlyEnemies",
    "SwarmingElites",
    "DoubleBoss",
];

/// The modifiers custom mode offers, by the ids the game spells them with.
/// Character Cards names the character whose cards it adds.
const MODIFIERS: [&str; 17] = [
    "ALL_STAR",
    "BIG_GAME_HUNTER",
    "CHARACTER_CARDS:DEFECT",
    "CHARACTER_CARDS:NECROBINDER",
    "CURSED_RUN",
    "DEADLY_EVENTS",
    "DRAFT",
    "FLIGHT",
    "HOARDER",
    "INSANITY",
    "MIDAS",
    "MURDEROUS",
    "NIGHT_TERRORS",
    "SEALED_DECK",
    "SPECIALIZED",
    "TERMINAL",
    "VINTAGE",
];

#[derive(clap::Args)]
pub struct Args {
    /// Seeds to play for each character.
    #[arg(long, default_value_t = 100)]
    runs: usize,
    /// The first seed's number. Each character plays `runs` seeds from it on.
    #[arg(long, default_value_t = 0)]
    first_seed: usize,
    /// A character to play, by id. Every playable one when none is named.
    #[arg(long = "character")]
    characters: Vec<String>,
    /// How many games play at once.
    #[arg(long, default_value_t = 32)]
    width: usize,
    /// The ascension level every run plays at, by the id the game spells it
    /// with.
    #[arg(long, default_value = "None", conflicts_with = "mixed")]
    ascension: String,
    /// The modifiers every run starts with, by the ids the game spells them
    /// with.
    #[arg(long, value_delimiter = ',', conflicts_with = "mixed")]
    modifiers: Vec<String>,
    /// Spread the seeds over every setup in turn: none, each ascension level,
    /// then each modifier.
    #[arg(long)]
    mixed: bool,
    /// Count the game's rules methods each run enters.
    #[arg(long)]
    coverage: bool,
}

/// One run to play.
struct Job {
    seed: String,
    character: String,
    /// The setup's name in the report, and the runner's flags for it.
    setup: (String, Vec<String>),
}

/// How one run went.
enum Went {
    Agreed,
    /// Kept, with the line that says where.
    Parted(String),
    /// The run did not play to an end, and why.
    Failed(String),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Process(#[from] crate::error::ProcessError),
    #[error(transparent)]
    File(#[from] crate::error::FileError),
    #[error(transparent)]
    Version(#[from] crate::codegen::VersionError),
}

pub fn run(args: Args) -> Result<String, Error> {
    let Args {
        runs,
        first_seed,
        mut characters,
        width,
        ascension,
        modifiers,
        mixed,
        coverage,
    } = args;
    if characters.is_empty() {
        characters = CHARACTERS.iter().map(|c| c.to_string()).collect();
    }
    let setups: Vec<(String, Vec<String>)> = match mixed {
        true => std::iter::once(("None".to_owned(), Vec::new()))
            .chain(
                ASCENSIONS
                    .iter()
                    .map(|&level| (level.to_owned(), Vec::new())),
            )
            .chain(
                MODIFIERS
                    .iter()
                    .map(|&modifier| ("None".to_owned(), vec![modifier.to_owned()])),
            )
            .map(|(level, modifiers)| (named(&level, &modifiers), setup(level, &modifiers)))
            .collect(),
        false => vec![(named(&ascension, &modifiers), setup(ascension, &modifiers))],
    };
    let jobs: Vec<Job> = characters
        .iter()
        .flat_map(|character| {
            let setups = &setups;
            (first_seed..first_seed + runs).map(move |at| Job {
                seed: crate::runner::seed(at),
                character: character.clone(),
                setup: setups[at % setups.len()].clone(),
            })
        })
        .collect();

    let root = crate::workspace();
    let version = crate::codegen::version()?;
    let runner = crate::error::build_runner(&root)?;
    let merger = crate::error::build_example(&root, "merge_runs")?;
    let scratch = root.join("target/hunt").join(&version);
    let _ = std::fs::remove_dir_all(&scratch);
    crate::error::create_dir(&scratch.join("runs"))?;
    let counts = coverage.then(|| crate::game_coverage::dir(&root, &version));
    if let Some(counts) = &counts {
        crate::error::create_dir(counts)?;
    }

    let retried = AtomicUsize::new(0);
    let went: Vec<(usize, Went)> = crate::runner::pool(&jobs, width, |worker, at, job| {
        let home = scratch.join(format!("home{worker}"));
        let out = run_file(&scratch, job);
        // Named for the run, so a run played again writes over its own
        // counts and never adds them twice.
        let counted = counts.as_ref().map(|dir| {
            let name = out.file_stem().expect("a run file has a name");
            dir.join(name).with_extension("cov")
        });
        let went = hunt_one(
            &runner,
            &version,
            job,
            &out,
            &home,
            counted.as_deref(),
            &retried,
        );
        (at, went)
    });

    let kept: Vec<PathBuf> = went
        .iter()
        .filter(|(_, went)| matches!(went, Went::Parted(_)))
        .map(|(at, _)| run_file(&scratch, &jobs[*at]))
        .filter(|file| file.is_file())
        .collect();
    let out = root
        .join("target/hunt")
        .join(format!("parted-{version}.bin"));
    let _ = std::fs::remove_file(&out);
    if !kept.is_empty() {
        crate::error::status(Command::new(&merger).arg(&out).args(&kept))?;
    }
    Ok(report(&jobs, &went, &out, retried.load(Ordering::Relaxed)))
}

/// A setup's name in the report.
fn named(ascension: &str, modifiers: &[String]) -> String {
    match modifiers {
        [] => ascension.to_owned(),
        _ => format!("{ascension} {}", modifiers.join(",")),
    }
}

/// The runner's flags for a run at `ascension` with `modifiers`.
fn setup(ascension: String, modifiers: &[String]) -> Vec<String> {
    crate::record::setup(ascension, modifiers)
}

/// Where one run is written, if it parts.
fn run_file(scratch: &Path, job: &Job) -> PathBuf {
    let setup = job.setup.0.replace([' ', ',', ':'], "-");
    scratch
        .join("runs")
        .join(format!("{}-{}-{setup}.bin", job.character, job.seed))
}

/// Plays one run beside the simulator, again after a crash. A run they part
/// on is kept, with the line that says where. One they agree on is thrown
/// away.
fn hunt_one(
    runner: &Path,
    build: &str,
    job: &Job,
    out: &Path,
    home: &Path,
    counted: Option<&Path>,
    retried: &AtomicUsize,
) -> Went {
    let mut args = crate::runner::args([job.seed.as_str(), &job.character, "--build", build]);
    args.extend(crate::runner::args([
        "--table",
        "game",
        "--carried",
        "--hunt",
    ]));
    args.extend(crate::runner::args(["--record".as_ref(), out.as_os_str()]));
    args.extend(crate::runner::args(["--home".as_ref(), home.as_os_str()]));
    args.extend(crate::runner::args(&job.setup.1));
    let env: Vec<(&str, std::ffi::OsString)> = counted
        .map(|counted| ("STS2_HEADLESS_COVERAGE", counted.as_os_str().to_owned()))
        .into_iter()
        .collect();
    let verdict = match crate::runner::retrying(runner, &args, &env, retried).verdict {
        Ok(verdict) => verdict,
        Err(why) => return Went::Failed(why),
    };
    match verdict.end {
        End::Agreed => {
            let _ = std::fs::remove_file(out);
            Went::Agreed
        }
        End::Parted => Went::Parted(format!("parted {} {}", verdict.step, verdict.detail)),
        End::Panicked => Went::Parted(format!("panicked {} {}", verdict.step, verdict.detail)),
        _ => {
            let _ = std::fs::remove_file(out);
            Went::Failed(verdict.to_string())
        }
    }
}

/// The counts by setup, each parted run, each failed one, and what the
/// agreed runs say about the rest.
fn report(jobs: &[Job], went: &[(usize, Went)], out: &Path, retried: usize) -> String {
    // Agreed, parted and failed, by setup.
    let mut by_setup: BTreeMap<&str, [usize; 3]> = BTreeMap::new();
    for (at, went) in went {
        let counts = by_setup.entry(jobs[*at].setup.0.as_str()).or_default();
        counts[match went {
            Went::Agreed => 0,
            Went::Parted(_) => 1,
            Went::Failed(_) => 2,
        }] += 1;
    }
    let [agreed, parted, failed] = by_setup.values().fold([0; 3], |sum, counts| {
        [sum[0] + counts[0], sum[1] + counts[1], sum[2] + counts[2]]
    });

    let mut report = String::new();
    writeln!(
        report,
        "{} runs: {agreed} agreed, {parted} parted, {failed} failed, {retried} played again after a crash",
        jobs.len()
    )
    .unwrap();
    if by_setup.len() > 1 {
        writeln!(report, "by setup: agreed, parted, failed").unwrap();
        for (setup, [agreed, parted, failed]) in &by_setup {
            writeln!(report, "  {setup:<36} {agreed:4} {parted:4} {failed:4}").unwrap();
        }
    }
    if parted == 0 && agreed > 0 {
        writeln!(
            report,
            "none parted: a run parts less often than 3 in {agreed}, at 95% confidence"
        )
        .unwrap();
    }
    if parted > 0 {
        writeln!(report, "parted, kept in {} in this order:", out.display()).unwrap();
        let mut index = 0;
        for (at, went) in went {
            if let Went::Parted(line) = went {
                let job = &jobs[*at];
                writeln!(
                    report,
                    "  {index:3} {} {} {}: {line}",
                    job.character, job.seed, job.setup.0
                )
                .unwrap();
                index += 1;
            }
        }
    }
    for (at, went) in went {
        if let Went::Failed(why) = went {
            let job = &jobs[*at];
            writeln!(
                report,
                "  failed {} {} {}: {why}",
                job.character, job.seed, job.setup.0
            )
            .unwrap();
        }
    }
    report
}

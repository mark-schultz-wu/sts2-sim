//! Which unported path stops the simulator's runs, and how often.
//!
//! Each run is `sts2-run` on the simulator, for one seed and character, in a
//! process of its own. It carries 25 Gorgets and a Mercury Hourglass, so a
//! run stops on what the simulator cannot play rather than on bad play. A
//! path nobody has ported panics, and the panic ends that run and nothing
//! else. The panics that stop the most runs are what to port first.
//!
//! ```text
//! cargo xtask census
//! cargo xtask census --runs 500 --character REGENT --top 30
//! cargo xtask census --seed C000000151 --character SILENT
//! ```
//!
//! `--seed` plays that one seed and prints how it ended, with the runner's
//! command line that plays it again.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::process::Command;

use sts2_core::{End, Verdict};

/// The characters a census plays when none is named.
const CHARACTERS: [&str; 5] = ["IRONCLAD", "SILENT", "REGENT", "NECROBINDER", "DEFECT"];

/// The acts a run passes through to win.
const ACTS: u32 = 3;

/// Seconds a run gets before it counts as stuck.
const TIMEOUT: &str = "120";

/// The census's run: the simulator, picking at random, every player carrying
/// the hunt's relics. v0.107.1, whatever `STS2_GAME_VERSION` says.
const RUNNER: [&str; 7] = [
    "--build",
    "v0.107.1",
    "--table",
    "sim",
    "--player",
    "random",
    "--carried",
];

/// A run the census itself did not see through, rather than one that
/// stopped in the simulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, thiserror::Error)]
enum Broken {
    #[error("launching the runner: {0}")]
    Launching(std::io::ErrorKind),
    #[error("the runner exited with {0:?}, and no panic")]
    Exited(Option<i32>),
}

/// A stop as the rankings group it: its message with the digits left out,
/// and where it was raised.
type Stop<'a> = (String, &'a str);

/// How one run stopped.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Ending {
    /// A panic, by its message, with where it was raised.
    Panicked {
        message: String,
        at: String,
    },
    /// An error the runner returned, by the message on its last line.
    Failed {
        message: String,
    },
    Won,
    Lost,
    /// The run took every decision it was given.
    Capped,
    /// The run outlived its timeout.
    Stuck,
    /// Anything else, which is a fault in the census.
    Broken(Broken),
}

/// One run's result.
struct Run {
    seed: String,
    character: String,
    ending: Ending,
    /// The act and floor it last stood on.
    reached: (u32, u32),
}

#[derive(clap::Args)]
pub struct Args {
    /// Seeds to play for each character.
    #[arg(long, default_value_t = 100)]
    runs: usize,
    /// How many of each ranking to print.
    #[arg(long, default_value_t = 20)]
    top: usize,
    /// A character to play, by id. Every playable one when none is named.
    #[arg(long = "character")]
    characters: Vec<String>,
    /// One seed to play, whose command line to replay it is printed.
    #[arg(long)]
    seed: Option<String>,
    /// The ascension level every run plays at, by the id the game spells it
    /// with.
    #[arg(long, default_value = "None")]
    ascension: String,
    /// The modifiers every run starts with, by the ids the game spells them
    /// with, such as `FLIGHT`.
    #[arg(long, value_delimiter = ',')]
    modifiers: Vec<String>,
}

pub fn run(args: Args) -> Result<String, crate::error::ProcessError> {
    let Args {
        runs,
        top,
        mut characters,
        seed,
        ascension,
        modifiers,
    } = args;
    let setup = crate::record::setup(ascension, &modifiers);
    if characters.is_empty() {
        characters = CHARACTERS.iter().map(|c| c.to_string()).collect();
    }

    let binary = crate::error::build_runner(&crate::workspace())?;

    if let Some(seed) = seed {
        return Ok(characters
            .iter()
            .map(|character| replay(&binary, &one(&binary, &seed, character, &setup), &setup))
            .collect());
    }

    // Every character plays the same seeds, so two characters differ by what
    // they carry and not by the map they drew.
    let jobs: Vec<(String, String)> = characters
        .iter()
        .flat_map(|character| (0..runs).map(move |at| (crate::runner::seed(at), character.clone())))
        .collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let done = crate::runner::pool(&jobs, threads, |_, _, (seed, character)| {
        one(&binary, seed, character, &setup)
    });
    Ok(report(&done, top))
}

/// One run of the runner, in a process of its own.
fn one(binary: &std::path::Path, seed: &str, character: &str, setup: &[String]) -> Run {
    let (ending, reached) = attempt(binary, seed, character, setup);
    Run {
        seed: seed.to_owned(),
        character: character.to_owned(),
        ending,
        reached,
    }
}

/// How `run` ended, and the command line that plays it there again.
fn replay(binary: &std::path::Path, run: &Run, setup: &[String]) -> String {
    format!(
        "{:?}\n{} play {} {} {} {}\n",
        run.ending,
        binary.display(),
        run.seed,
        run.character,
        RUNNER.join(" "),
        setup.join(" ")
    )
}

/// How the runner ended, and the act and floor it last stood on.
fn attempt(
    binary: &std::path::Path,
    seed: &str,
    character: &str,
    setup: &[String],
) -> (Ending, (u32, u32)) {
    let output = Command::new("timeout")
        .args(["-k", "5", TIMEOUT])
        .arg(binary)
        .args(["play", seed, character])
        .args(RUNNER)
        .args(setup)
        .output();
    let output = match output {
        Ok(output) => output,
        Err(e) => return (Ending::Broken(Broken::Launching(e.kind())), (0, 0)),
    };
    if let Some(124 | 137) = output.status.code() {
        return (Ending::Stuck, (0, 0));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(verdict) = Verdict::last_in(&stdout) else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let ending = match stderr
            .lines()
            .last()
            .and_then(|last| last.strip_prefix("error: "))
        {
            Some(message) => Ending::Failed {
                message: message.to_owned(),
            },
            None => Ending::Broken(Broken::Exited(output.status.code())),
        };
        return (ending, (0, 0));
    };
    let ending = match verdict.end {
        End::Won => Ending::Won,
        End::Lost => Ending::Lost,
        End::Capped => Ending::Capped,
        End::Panicked => Ending::Panicked {
            message: verdict.detail,
            at: verdict.place.unwrap_or_default(),
        },
        end => Ending::Failed {
            message: format!("{end:?}: {}", verdict.detail),
        },
    };
    (ending, (verdict.act, verdict.floor))
}

/// The census as a table: how the runs stopped, then the panics by count.
fn report(runs: &[Run], top: usize) -> String {
    let total = runs.len();
    let count = |wanted: fn(&Ending) -> bool| runs.iter().filter(|run| wanted(&run.ending)).count();
    let mut out = format!(
        "{total} runs: {} won, {} lost, {} capped, {} stuck, {} broken, {} panicked, {} failed\n",
        count(|e| *e == Ending::Won),
        count(|e| *e == Ending::Lost),
        count(|e| *e == Ending::Capped),
        count(|e| *e == Ending::Stuck),
        count(|e| matches!(e, Ending::Broken(_))),
        count(|e| matches!(e, Ending::Panicked { .. })),
        count(|e| matches!(e, Ending::Failed { .. })),
    );

    // How far runs got, by character, as the floors they stood on last.
    let mut by_character: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    for run in runs {
        by_character
            .entry(&run.character)
            .or_default()
            .push(run.reached.1);
    }
    out.push_str("floor reached, by character: lowest, median, best\n");
    for (character, mut floors) in by_character {
        floors.sort_unstable();
        writeln(
            &mut out,
            format!(
                "  {character:<12} {:>3} {:>3} {:>3}",
                floors[0],
                floors[floors.len() / 2],
                floors.last().copied().unwrap_or(0)
            ),
        );
    }

    // The goal: each character's lowest stop, and what stopped runs there.
    // A loss is apart, since no port raises the floor a run died on.
    out.push_str("lowest stop, by character: floor, runs stopped there, lowest loss\n");
    let mut characters: Vec<&str> = runs.iter().map(|run| run.character.as_str()).collect();
    characters.sort_unstable();
    characters.dedup();
    for character in characters {
        let theirs = || runs.iter().filter(move |run| run.character == character);
        let lost = theirs()
            .filter(|run| run.ending == Ending::Lost)
            .map(|run| run.reached.1)
            .min()
            .map_or_else(|| "none".to_owned(), |floor| floor.to_string());
        let stopped: Vec<&Run> = theirs()
            .filter(|run| matches!(run.ending, Ending::Panicked { .. } | Ending::Failed { .. }))
            .collect();
        let Some(floor) = stopped.iter().map(|run| run.reached.1).min() else {
            writeln(&mut out, format!("  {character:<12}   -     0  {lost:>4}"));
            continue;
        };
        let lowest: Vec<&Run> = stopped
            .into_iter()
            .filter(|run| run.reached.1 == floor)
            .collect();
        writeln(
            &mut out,
            format!(
                "  {character:<12} {floor:>3} {:>5}  {lost:>4}",
                lowest.len()
            ),
        );
        let mut by_stop: BTreeMap<String, (usize, &Run)> = BTreeMap::new();
        for run in lowest {
            let message = match &run.ending {
                Ending::Panicked { message, .. } | Ending::Failed { message } => message,
                _ => continue,
            };
            let key: String = message.chars().filter(|c| !c.is_ascii_digit()).collect();
            by_stop.entry(key).or_insert((0, run)).0 += 1;
        }
        let mut by_stop: Vec<_> = by_stop.into_iter().collect();
        by_stop.sort_by_key(|(_, (count, _))| std::cmp::Reverse(*count));
        for (message, (count, run)) in by_stop.into_iter().take(top) {
            writeln(
                &mut out,
                format!("      {count:>5}  {message}  e.g. {}", run.seed),
            );
        }
    }

    // How many runs cleared each act: stood in a later one, or won.
    out.push_str("acts cleared, of every run\n");
    for act in 0..ACTS {
        let cleared = runs
            .iter()
            .filter(|run| run.reached.0 > act || run.ending == Ending::Won)
            .count();
        writeln(
            &mut out,
            format!(
                "  act {:<8} {cleared:>5}/{:<5} {:>3}%",
                act + 1,
                runs.len(),
                100 * cleared / runs.len().max(1)
            ),
        );
    }

    // What holds the lowest runs down: each stop by the lowest floor it
    // stopped a run on, then by how many runs it stopped there.
    let mut lowest: BTreeMap<Stop, (u32, usize, Cow<str>, &Run)> = BTreeMap::new();
    for run in runs {
        let (message, at) = match &run.ending {
            Ending::Panicked { message, at } => (Cow::Borrowed(message.as_str()), at.as_str()),
            Ending::Failed { message } => (Cow::Borrowed(message.as_str()), ""),
            Ending::Broken(why) => (Cow::Owned(why.to_string()), ""),
            _ => continue,
        };
        let key: String = message.chars().filter(|c| !c.is_ascii_digit()).collect();
        let floor = run.reached.1;
        let entry = lowest
            .entry((key, at))
            .or_insert((floor, 0, message.clone(), run));
        if floor < entry.0 {
            *entry = (floor, 0, message, run);
        }
        if floor == entry.0 {
            entry.1 += 1;
        }
    }
    let mut lowest: Vec<_> = lowest.into_values().collect();
    lowest.sort_by_key(|(floor, count, _, _)| (*floor, std::cmp::Reverse(*count)));
    out.push_str(&format!(
        "what stops the lowest runs, the {} lowest: floor, runs stopped there\n",
        top.min(lowest.len())
    ));
    for (floor, count, message, run) in lowest.into_iter().take(top) {
        writeln(
            &mut out,
            format!(
                "{floor:>5} {count:>5}  {message}\n             e.g. {} {}",
                run.seed, run.character
            ),
        );
    }

    let stopped = runs.iter().map(|run| (&run.ending, run));
    table(
        &mut out,
        "panics and errors that stopped a run",
        stopped,
        top,
    );
    // The same, for the runs that stopped in each act, which is where a
    // goal for one act reads.
    for act in 0..ACTS {
        let stopped: Vec<_> = runs
            .iter()
            .filter(|run| run.reached.0 == act)
            .map(|run| (&run.ending, run))
            .collect();
        if stopped.is_empty() {
            continue;
        }
        table(
            &mut out,
            &format!("panics and errors that stopped a run in act {}", act + 1),
            stopped.into_iter(),
            top,
        );
    }
    out
}

/// Panics by message, commonest first, with the runs each one hit.
fn table<'a>(
    out: &mut String,
    title: &str,
    endings: impl Iterator<Item = (&'a Ending, &'a Run)>,
    top: usize,
) {
    // Keyed with the digits left out, so the same hole reached at another
    // creature or amount is one row. The row shows the first message.
    let mut panics: BTreeMap<Stop, (Cow<str>, Vec<&Run>)> = BTreeMap::new();
    for (ending, run) in endings {
        let (message, at) = match ending {
            Ending::Panicked { message, at } => (Cow::Borrowed(message.as_str()), at.as_str()),
            Ending::Failed { message } => (Cow::Borrowed(message.as_str()), ""),
            Ending::Broken(why) => (Cow::Owned(why.to_string()), ""),
            _ => continue,
        };
        let key: String = message.chars().filter(|c| !c.is_ascii_digit()).collect();
        panics
            .entry((key, at))
            .or_insert_with(|| (message, Vec::new()))
            .1
            .push(run);
    }
    let mut panics: Vec<_> = panics
        .into_iter()
        .map(|((_, at), (message, runs))| ((message, at), runs))
        .collect();
    panics.sort_by_key(|(_, runs)| std::cmp::Reverse(runs.len()));
    out.push_str(&format!(
        "{title}, the {} commonest of {}:\n",
        top.min(panics.len()),
        panics.len()
    ));
    for ((message, at), stopped) in panics.into_iter().take(top) {
        let mut characters: Vec<&str> = stopped.iter().map(|run| run.character.as_str()).collect();
        characters.sort_unstable();
        characters.dedup();
        let first = stopped[0];
        writeln(
            out,
            format!(
                "{:>5}  {message}\n       {at}\n       {}, e.g. {} {}",
                stopped.len(),
                characters.join(" "),
                first.seed,
                first.character
            ),
        );
    }
}

fn writeln(out: &mut String, line: String) {
    out.push_str(&line);
    out.push('\n');
}

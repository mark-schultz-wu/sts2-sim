//! Which unported path stops the simulator's runs, and how often.
//!
//! Each run is `sts2-golden`'s `random_run` for one seed and character, in a
//! process of its own. It carries 25 Gorgets and a Mercury Hourglass, so a
//! run stops on what the simulator cannot play rather than on bad play. A path nobody has ported panics, and the panic ends that
//! process and nothing else.
//!
//! A panic inside a decision the run had alternatives to is one a player
//! steps round. The census plays the run again with that move avoided, until
//! the run ends or every move at some step panics. It reports both kinds.
//! A panic that stopped a run is a hole in what the simulator can play. A
//! panic a run stepped round is a choice an agent cannot make.
//!
//! ```text
//! cargo xtask census
//! cargo xtask census --runs 500 --character REGENT --top 30
//! ```

use std::collections::BTreeMap;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::instructions::workspace;

/// The characters a census plays when none is named.
const CHARACTERS: [&str; 5] = ["IRONCLAD", "SILENT", "REGENT", "NECROBINDER", "DEFECT"];

/// Seconds a run gets before it counts as stuck.
const TIMEOUT: &str = "120";

/// The moves one run steps round before the census gives up on it.
const RETRIES: usize = 200;

/// How one run stopped.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Ending {
    /// A panic, by its message, with where it was raised.
    Panicked {
        message: String,
        at: String,
    },
    Won,
    Lost,
    /// The run took every decision it was given.
    Capped,
    /// The run outlived its timeout.
    Stuck,
    /// Anything else, which is a fault in the census.
    Broken(String),
}

/// One run's result.
struct Run {
    seed: String,
    character: String,
    ending: Ending,
    /// The act and floor it last stood on.
    reached: (u32, u32),
    /// The panics it stepped round on the way, each a move it avoided, by
    /// the step it was avoided at.
    avoided: Vec<(usize, Ending)>,
    /// The step whose every move panicked, when that is how it stopped.
    exhausted: Option<usize>,
}

impl Run {
    /// What stopped the run: every panic at the step that ran out of moves,
    /// or else how it ended.
    fn stoppers(&self) -> Vec<&Ending> {
        match self.exhausted {
            Some(step) => self
                .avoided
                .iter()
                .filter(|(at, _)| *at == step)
                .map(|(_, ending)| ending)
                .chain([&self.ending])
                .collect(),
            None => vec![&self.ending],
        }
    }

    /// The panics it stepped round and went on from.
    fn stepped_round(&self) -> impl Iterator<Item = &Ending> {
        self.avoided
            .iter()
            .filter(|(at, _)| Some(*at) != self.exhausted)
            .map(|(_, ending)| ending)
    }
}

/// One attempt at a run, and where it panicked when it did.
struct Attempt {
    ending: Ending,
    reached: (u32, u32),
    /// The step whose decision was running when it stopped, the move it
    /// took there, and how many moves that step had.
    stopped_in: Option<(usize, usize, usize)>,
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
}

pub fn run(args: Args) -> Result<String, String> {
    let Args {
        runs,
        top,
        mut characters,
    } = args;
    if characters.is_empty() {
        characters = CHARACTERS.iter().map(|c| c.to_string()).collect();
    }

    let root = workspace()?;
    let built = Command::new("cargo")
        .args([
            "build",
            "-q",
            "--release",
            "-p",
            "sts2-golden",
            "--example",
            "random_run",
        ])
        .current_dir(&root)
        .status()
        .map_err(|e| format!("running cargo: {e}"))?;
    if !built.success() {
        return Err("the runner did not build".into());
    }
    let binary = root.join("target/release/examples/random_run");

    // Every character plays the same seeds, so two characters differ by what
    // they carry and not by the map they drew.
    let jobs: Vec<(String, String)> = characters
        .iter()
        .flat_map(|character| (0..runs).map(move |at| (format!("C{at:09}"), character.clone())))
        .collect();
    let next = AtomicUsize::new(0);
    let done = Mutex::new(Vec::with_capacity(jobs.len()));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while let Some((seed, character)) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let run = one(&binary, seed, character);
                    done.lock().expect("no thread panics").push(run);
                }
            });
        }
    });
    let done = done.into_inner().expect("no thread panics");
    Ok(report(&done, top))
}

/// One run, played again round every move that panicked while the step
/// that took it had others.
fn one(binary: &std::path::Path, seed: &str, character: &str) -> Run {
    let mut avoid: Vec<(usize, usize)> = Vec::new();
    let mut avoided = Vec::new();
    loop {
        let attempt = attempt(binary, seed, character, &avoid);
        let (step, exhausted) = match (&attempt.ending, attempt.stopped_in) {
            (Ending::Panicked { .. }, Some((step, at, moves))) if avoid.len() < RETRIES => {
                avoid.push((step, at));
                let left = (0..moves)
                    .filter(|at| !avoid.contains(&(step, *at)))
                    .count();
                (Some(step), left == 0)
            }
            _ => (None, false),
        };
        match step {
            Some(step) if !exhausted => avoided.push((step, attempt.ending)),
            _ => {
                return Run {
                    seed: seed.to_owned(),
                    character: character.to_owned(),
                    ending: attempt.ending,
                    reached: attempt.reached,
                    avoided,
                    exhausted: step,
                };
            }
        }
    }
}

/// One process of the runner, with the moves it avoids.
fn attempt(
    binary: &std::path::Path,
    seed: &str,
    character: &str,
    avoid: &[(usize, usize)],
) -> Attempt {
    let output = Command::new("timeout")
        .args(["-k", "5", TIMEOUT])
        .arg(binary)
        .args([seed, character])
        .args(
            avoid
                .iter()
                .flat_map(|(step, at)| ["--avoid".to_owned(), format!("{step}:{at}")]),
        )
        .output();
    let output = match output {
        Ok(output) => output,
        Err(e) => {
            return Attempt {
                ending: Ending::Broken(format!("launching: {e}")),
                reached: (0, 0),
                stopped_in: None,
            };
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let reached = stdout
        .lines()
        .rev()
        .filter_map(|line| line.strip_prefix("at "))
        .find_map(|rest| {
            let (act, floor) = rest.split_once(' ')?;
            Some((act.parse().ok()?, floor.parse().ok()?))
        })
        .unwrap_or((0, 0));
    // A `step` line with no `done` after it is the decision that stopped.
    let mut stopped_in = None;
    for line in stdout.lines() {
        match line.split_whitespace().collect::<Vec<_>>()[..] {
            ["step", step, at, moves] => {
                stopped_in =
                    (|| Some((step.parse().ok()?, at.parse().ok()?, moves.parse().ok()?)))()
            }
            ["done"] => stopped_in = None,
            _ => {}
        }
    }
    let last = stdout.lines().last().unwrap_or_default();
    let ending = match (output.status.code(), last) {
        (Some(0), "over won") => Ending::Won,
        (Some(0), "over lost") => Ending::Lost,
        (Some(0), "capped") => Ending::Capped,
        (Some(124 | 137), _) => Ending::Stuck,
        _ => {
            panicked(&stderr).unwrap_or_else(|| Ending::Broken(format!("exited {}", output.status)))
        }
    };
    Attempt {
        ending,
        reached,
        stopped_in,
    }
}

/// The panic stderr reports: `thread '...' panicked at <where>:` and then the
/// message on the lines after it, up to the backtrace note.
fn panicked(stderr: &str) -> Option<Ending> {
    let mut lines = stderr.lines();
    let at = lines
        .by_ref()
        .find_map(|line| line.split_once(" panicked at ").map(|(_, at)| at))?;
    let message = lines
        .take_while(|line| !line.starts_with("note: "))
        .collect::<Vec<_>>()
        .join(" ");
    Some(Ending::Panicked {
        message,
        at: at.trim_end_matches(':').to_owned(),
    })
}

/// The census as a table: how the runs stopped, then the panics by count.
fn report(runs: &[Run], top: usize) -> String {
    let total = runs.len();
    let count = |wanted: fn(&Ending) -> bool| runs.iter().filter(|run| wanted(&run.ending)).count();
    let mut out = format!(
        "{total} runs: {} won, {} lost, {} capped, {} stuck, {} broken, {} panicked\n",
        count(|e| *e == Ending::Won),
        count(|e| *e == Ending::Lost),
        count(|e| *e == Ending::Capped),
        count(|e| *e == Ending::Stuck),
        count(|e| matches!(e, Ending::Broken(_))),
        count(|e| matches!(e, Ending::Panicked { .. })),
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

    // What holds the lowest runs down: each stopper by the lowest floor it
    // stopped a run on, then by how many runs it stopped there.
    let mut lowest: BTreeMap<(String, &str), (u32, usize, &str, &Run)> = BTreeMap::new();
    for run in runs {
        for ending in run.stoppers() {
            let (message, at) = match ending {
                Ending::Panicked { message, at } => (message.as_str(), at.as_str()),
                Ending::Broken(why) => (why.as_str(), ""),
                _ => continue,
            };
            let key: String = message.chars().filter(|c| !c.is_ascii_digit()).collect();
            let floor = run.reached.1;
            let entry = lowest.entry((key, at)).or_insert((floor, 0, message, run));
            if floor < entry.0 {
                *entry = (floor, 0, message, run);
            }
            if floor == entry.0 {
                entry.1 += 1;
            }
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

    // A run that ran out of moves counts against every panic at that step.
    let stopped = runs
        .iter()
        .flat_map(|run| run.stoppers().into_iter().map(move |ending| (ending, run)));
    table(&mut out, "panics that stopped a run", stopped, top);
    let avoided = runs
        .iter()
        .flat_map(|run| run.stepped_round().map(move |ending| (ending, run)));
    table(&mut out, "panics a run stepped round", avoided, top);
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
    let mut panics: BTreeMap<(String, &str), (&str, Vec<&Run>)> = BTreeMap::new();
    for (ending, run) in endings {
        let (message, at) = match ending {
            Ending::Panicked { message, at } => (message.as_str(), at.as_str()),
            Ending::Broken(why) => (why.as_str(), ""),
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

//! How far the simulator follows the runs the game played, for `coverage`.
//!
//! Each run in the build's recorded file, [`recorded_file`], is followed
//! by `sts2-run follow`, in a process of its own so a panic names
//! its run. Then, for v0.107.1, whose game played it, the long run
//! 0RACLE1-REGENT-2, by its `BASELINE`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use sts2_core::{End, Verdict};

use crate::codegen::VersionError;
use crate::coverage::Error;

/// How many of the commonest first splits the report names.
const SPLITS: usize = 20;

/// The long run the divergence test replays.
const LONG_RUN: &str = "0RACLE1-REGENT-2";

/// One run, as `sts2-run follow` left it.
struct Line {
    character: String,
    /// The positions that agreed, and how many the run has.
    matched: usize,
    positions: usize,
    /// The move before the first split, or the panic that cut it.
    split: Option<String>,
}

/// One character's runs, summed.
#[derive(Default)]
struct Tally {
    runs: usize,
    whole: usize,
    matched: usize,
    positions: usize,
}

/// The file of runs recorded on the build `STS2_GAME_VERSION` names.
/// v0.107.1's has the plain name.
pub(crate) fn recorded_file(root: &Path) -> Result<PathBuf, VersionError> {
    let file = match crate::codegen::version()?.as_str() {
        "v0.107.1" => "recorded.bin".to_owned(),
        other => format!("recorded-{other}.bin"),
    };
    Ok(root.join("crates/sts2-golden/tests").join(file))
}

pub fn report(root: &Path, out: &mut String) -> Result<(), Error> {
    let follow = crate::error::build_runner(root)?;
    let file = recorded_file(root)?;
    let counted = crate::error::output(Command::new(&follow).arg("follow").arg(&file))?;
    let runs: usize = String::from_utf8_lossy(&counted.stdout)
        .trim()
        .parse()
        .expect("`sts2-run follow` prints how many runs there are");

    let next = AtomicUsize::new(0);
    let lines = Mutex::new(Vec::with_capacity(runs));
    let width = std::thread::available_parallelism().map_or(4, |n| n.get());
    let failed = Mutex::new(None);
    std::thread::scope(|scope| {
        for _ in 0..width {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= runs {
                        break;
                    }
                    match crate::error::spawned(
                        Command::new(&follow)
                            .arg("follow")
                            .arg(&file)
                            .arg(index.to_string()),
                    ) {
                        Ok(output) => {
                            let line = read(&output);
                            lines.lock().expect("no thread panics").push(line);
                        }
                        Err(e) => *failed.lock().expect("no thread panics") = Some(e),
                    }
                }
            });
        }
    });
    if let Some(e) = failed.into_inner().expect("no thread panics") {
        return Err(e.into());
    }
    let lines = lines.into_inner().expect("no thread panics");

    let mut by_character: BTreeMap<&str, Tally> = BTreeMap::new();
    let mut splits: BTreeMap<&str, usize> = BTreeMap::new();
    for line in &lines {
        let tally = by_character.entry(&line.character).or_default();
        tally.runs += 1;
        tally.matched += line.matched;
        tally.positions += line.positions;
        match &line.split {
            None => tally.whole += 1,
            Some(split) => *splits.entry(split).or_default() += 1,
        }
    }

    writeln!(
        out,
        "\nrecorded runs the simulator follows, of {}",
        file.strip_prefix(root).unwrap_or(&file).display()
    )
    .unwrap();
    let mut all = Tally::default();
    for (character, tally) in &by_character {
        row(out, character, tally);
        all.runs += tally.runs;
        all.whole += tally.whole;
        all.matched += tally.matched;
        all.positions += tally.positions;
    }
    row(out, "all", &all);

    if crate::codegen::version()? == "v0.107.1" {
        let golden = root.join("crates/sts2-golden/tests/golden").join(LONG_RUN);
        let baseline = crate::error::read(&golden.join("BASELINE"))?;
        let steps = crate::error::read(&golden.join("digests"))?.lines().count();
        writeln!(out, "  {LONG_RUN}  {} of {steps} steps", baseline.trim()).unwrap();
    }

    let mut splits: Vec<(&str, usize)> = splits.into_iter().collect();
    splits.sort_by_key(|(split, count)| (std::cmp::Reverse(*count), *split));
    writeln!(
        out,
        "\nthe commonest first splits, by the move that led to them"
    )
    .unwrap();
    for (split, count) in splits.iter().take(SPLITS) {
        writeln!(out, "  {count:>4}  {split}").unwrap();
    }
    Ok(())
}

fn row(out: &mut String, name: &str, tally: &Tally) {
    let percent = match tally.positions {
        0 => 100,
        positions => 100 * tally.matched / positions,
    };
    writeln!(
        out,
        "  {name:<12} {:>4}/{:<4} whole  {:>8}/{:<8} positions {percent:>3}%",
        tally.whole, tally.runs, tally.matched, tally.positions
    )
    .unwrap();
}

/// What `sts2-run follow` said of one run: `run <characters> <positions>`,
/// then its verdict, whose step is the positions that agreed.
fn read(output: &std::process::Output) -> Line {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let said = || {
        format!(
            "`sts2-run follow` printed {stdout:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    };
    let (character, positions) = stdout
        .lines()
        .find_map(|line| {
            let mut words = line.strip_prefix("run ")?.split_whitespace();
            Some((words.next()?.to_owned(), words.next()?.parse().ok()?))
        })
        .unwrap_or_else(|| panic!("{}", said()));
    let verdict = Verdict::last_in(&stdout).unwrap_or_else(|| panic!("{}", said()));
    let split = match verdict.end {
        End::Agreed => None,
        End::Parted => Some(verdict.detail),
        End::Panicked => Some(format!("panicked: {}", verdict.detail)),
        end => Some(format!("{end}: {}", verdict.detail)),
    };
    Line {
        character,
        matched: verdict.step,
        positions,
        split,
    }
}

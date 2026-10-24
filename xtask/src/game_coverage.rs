//! `cargo xtask game-coverage`: which of the game's own rules methods the
//! hunts entered.
//!
//! `reached` counts the simulator's rules code, so it cannot see a rule the
//! simulator never ported. This counts the game's. `cargo xtask hunt
//! --coverage` has the mod count each rules method a run enters, one file per
//! run under `target/game-coverage/<build>/`. A run played again writes over
//! its own file, so no run counts twice.
//!
//! This adds the files up by area. `--under` narrows it to the methods whose
//! name starts so, and `--unentered` lists the ones no run entered.
//!
//! ```text
//! cargo xtask hunt --runs 100 --first-seed 1000 --mixed --coverage
//! cargo xtask game-coverage
//! cargo xtask game-coverage --under Models.Cards --unentered
//! ```
//!
//! The count is per method, and a method the JIT inlined into a caller
//! before the mod patched it counts as never entered. The build is
//! `STS2_GAME_VERSION`'s.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::error::{self, FileError};

/// What every method's name starts with.
const ROOT: &str = "MegaCrit.Sts2.Core.";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    Version(#[from] crate::codegen::VersionError),
    #[error("no counts in {0}; run `cargo xtask hunt --coverage` first")]
    Nothing(PathBuf),
    #[error("{file}:{line}: not a count and a method")]
    Malformed { file: PathBuf, line: usize },
}

#[derive(clap::Args)]
pub struct Args {
    /// Only the methods whose name, less `MegaCrit.Sts2.Core.`, starts so.
    #[arg(long)]
    under: Option<String>,
    /// List the methods no run entered.
    #[arg(long)]
    unentered: bool,
}

/// Where the hunts write a build's counts.
pub fn dir(root: &Path, version: &str) -> PathBuf {
    root.join("target/game-coverage").join(version)
}

pub fn run(Args { under, unentered }: Args) -> Result<String, Error> {
    let root = crate::workspace();
    let version = crate::codegen::version()?;
    let dir = dir(&root, &version);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| path.extension().is_some_and(|ext| ext == "cov"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    if files.is_empty() {
        return Err(Error::Nothing(dir));
    }

    // Each method, less the root, with how many times the runs entered it.
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for file in &files {
        for (at, line) in error::read(file)?.lines().enumerate() {
            let (count, method) = line
                .split_once('\t')
                .and_then(|(count, method)| Some((count.parse::<u64>().ok()?, method)))
                .ok_or_else(|| Error::Malformed {
                    file: file.clone(),
                    line: at + 1,
                })?;
            let method = method.strip_prefix(ROOT).unwrap_or(method);
            *counts.entry(method.to_owned()).or_default() += count;
        }
    }
    let under = under
        .as_deref()
        .map(|under| under.strip_prefix(ROOT).unwrap_or(under));
    counts.retain(|method, _| under.is_none_or(|under| method.starts_with(under)));

    // Entered and in all, by area: the first two parts of the name.
    let mut areas: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for (method, &count) in &counts {
        let area = method.splitn(3, '.').take(2).collect::<Vec<_>>().join(".");
        let (entered, all) = areas.entry(area).or_default();
        *all += 1;
        *entered += usize::from(count > 0);
    }
    let entered = counts.values().filter(|&&count| count > 0).count();

    let mut report = String::new();
    writeln!(
        report,
        "{} runs on {version}: {entered} of {} methods entered ({:.1}%)",
        files.len(),
        counts.len(),
        percent(entered, counts.len()),
    )
    .unwrap();
    // The areas with most left out first.
    let mut areas: Vec<_> = areas.into_iter().collect();
    areas.sort_by_key(|(area, (entered, all))| (std::cmp::Reverse(all - entered), area.clone()));
    writeln!(report, "by area: never entered, entered, in all").unwrap();
    for (area, (entered, all)) in &areas {
        writeln!(
            report,
            "  {area:<40} {:>6} {entered:>6} {all:>6}  {:.1}%",
            all - entered,
            percent(*entered, *all),
        )
        .unwrap();
    }
    if unentered {
        writeln!(report, "never entered:").unwrap();
        for method in counts
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(m, _)| m)
        {
            writeln!(report, "  {method}").unwrap();
        }
    }
    Ok(report)
}

fn percent(part: usize, whole: usize) -> f64 {
    match whole {
        0 => 0.0,
        _ => 100.0 * part as f64 / whole as f64,
    }
}

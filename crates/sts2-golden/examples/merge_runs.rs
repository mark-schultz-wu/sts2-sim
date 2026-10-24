//! Many files of recorded runs as one, for `cargo xtask record`.
//!
//! ```text
//! merge_runs <out> <file>[:<index>,...]...
//! ```
//!
//! The runs go in the order the files are named. Every file has to come from
//! one build of the game, because a set of runs is compared as a set. A file
//! named with indices gives only those runs, in that order. That takes the
//! runs of a hunt's parted file that a fix made whole.

use std::path::PathBuf;

use clap::Parser;
use sts2_golden::recorded::Recorded;

/// Merge files of recorded runs.
#[derive(Parser)]
struct Cli {
    /// Where the merged file goes.
    out: PathBuf,
    /// The files to merge, in order, each with the indices of its runs to
    /// take where not all.
    files: Vec<String>,
}

fn main() -> Result<(), String> {
    let Cli { out, files } = Cli::parse();
    let mut merged: Option<Recorded> = None;
    for named in &files {
        let (file, picked) = match named.rsplit_once(':') {
            Some((file, indices)) => {
                let picked = indices
                    .split(',')
                    .map(|index| index.parse::<usize>().map_err(|e| format!("{named}: {e}")))
                    .collect::<Result<Vec<_>, _>>()?;
                (PathBuf::from(file), Some(picked))
            }
            None => (PathBuf::from(named), None),
        };
        let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let mut read = Recorded::decode(&bytes).map_err(|e| format!("{}: {e}", file.display()))?;
        if let Some(picked) = picked {
            read.runs = picked
                .iter()
                .map(|&index| {
                    read.runs
                        .get(index)
                        .cloned()
                        .ok_or_else(|| format!("{named}: no run {index}"))
                })
                .collect::<Result<_, _>>()?;
        }
        match &mut merged {
            None => merged = Some(read),
            Some(merged) if merged.stamp == read.stamp => merged.runs.extend(read.runs),
            Some(merged) => {
                return Err(format!(
                    "{} came from another build: {:?}, not {:?}",
                    file.display(),
                    read.stamp,
                    merged.stamp
                ));
            }
        }
    }
    let merged = merged.ok_or("no files to merge")?;
    std::fs::write(&out, merged.encode()).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("{} runs into {}", merged.runs.len(), out.display());
    Ok(())
}

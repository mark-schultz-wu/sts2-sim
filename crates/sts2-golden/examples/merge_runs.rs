//! Many files of recorded runs as one, for `cargo xtask record`.
//!
//! ```text
//! merge_runs <out> <file>...
//! ```
//!
//! The runs go in the order the files are named. Every file has to come from
//! one build of the game, because a set of runs is compared as a set.

use std::path::PathBuf;

use clap::Parser;
use sts2_golden::recorded::Recorded;

/// Merge files of recorded runs.
#[derive(Parser)]
struct Cli {
    /// Where the merged file goes.
    out: PathBuf,
    /// The files to merge, in order.
    files: Vec<PathBuf>,
}

fn main() -> Result<(), String> {
    let Cli { out, files } = Cli::parse();
    let mut merged: Option<Recorded> = None;
    for file in &files {
        let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
        let read = Recorded::decode(&bytes).map_err(|e| format!("{}: {e}", file.display()))?;
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

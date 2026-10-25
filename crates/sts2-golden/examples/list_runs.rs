//! The runs a file of recorded runs holds, one a line, for `cargo xtask
//! record --again`.
//!
//! ```text
//! list_runs <file>
//! ```
//!
//! Each line is the seed, the character, the ascension and the modifiers
//! (`-` for none), separated by tabs. That is everything `record_run` needs
//! to play the run again. The moves are the game's random picks from the
//! seed, so a run played again picks the same ones while its lists match.

use std::path::PathBuf;

use clap::Parser;
use sts2_golden::recorded::Recorded;

/// List the runs in a file of recorded runs.
#[derive(Parser)]
struct Cli {
    /// The file of runs.
    file: PathBuf,
}

fn main() -> Result<(), String> {
    let Cli { file } = Cli::parse();
    let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let read = Recorded::decode(&bytes).map_err(|e| format!("{}: {e}", file.display()))?;
    for run in &read.runs {
        let modifiers = match run.modifiers.is_empty() {
            true => "-".to_owned(),
            false => run.modifiers.join(","),
        };
        println!(
            "{}\t{}\t{}\t{modifiers}",
            run.seed,
            run.characters(),
            run.ascension
        );
    }
    Ok(())
}

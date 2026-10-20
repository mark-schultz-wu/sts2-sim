//! `cargo xtask split <INDEX> <NAME>`: a recorded run, up to its first split,
//! as a scenario.
//!
//! Up to its first split the simulator's legal list is the game's, so the
//! moves it plays there are the moves the game played. `follow_run
//! --scenario` spells them, and this writes them to
//! `tests/scenarios/<NAME>.txt`. Then the game is asked for the scenario like
//! any other, and `cargo xtask scenario-diff <NAME>` names what parts.
//!
//! `--file` reads another file of runs, such as `cargo xtask hunt`'s.
//!
//! The file is for finding the split. What lands with a fix is a scenario cut
//! down to the rule.

use std::fmt::Write as _;
use std::process::Command;

use crate::error::{self, FileError, ProcessError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    Version(#[from] crate::codegen::VersionError),
}

#[derive(clap::Args)]
pub struct Args {
    /// The run, by its place in the recorded file, counted from zero.
    index: usize,
    /// The scenario's name, which is its file's name without `.txt`.
    name: String,
    /// The file of runs, such as the parted runs `cargo xtask hunt` keeps.
    /// The build's recorded file when none is named.
    #[arg(long)]
    file: Option<std::path::PathBuf>,
}

pub fn run(Args { index, name, file }: Args) -> Result<String, Error> {
    let root = crate::workspace();
    let follow = error::build_example(&root, "follow_run")?;
    let file = match file {
        Some(file) => file,
        None => crate::followed::recorded_file(&root)?,
    };
    let at = index.to_string();
    let followed = error::output(Command::new(&follow).arg(&file).arg(&at))?;
    let spelled = error::output(Command::new(&follow).arg(&file).arg(&at).arg("--scenario"))?;

    let mut scenario = String::new();
    let said = String::from_utf8_lossy(&followed.stdout);
    writeln!(
        scenario,
        "# Recorded run {index}, to its first split: {}",
        said.trim()
    )
    .unwrap();
    scenario.push_str(&String::from_utf8_lossy(&spelled.stdout));
    let out = root
        .join("crates/sts2-golden/tests/scenarios")
        .join(format!("{name}.txt"));
    error::write(&out, scenario)?;
    Ok(format!("{}\n", out.display()))
}

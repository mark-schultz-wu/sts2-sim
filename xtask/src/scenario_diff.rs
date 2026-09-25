//! `cargo xtask scenario-diff <NAME>`: where a scenario's simulator and game
//! part.
//!
//! Runs the `scenario_diff` example, which walks both sides off the game's
//! cached transcript and writes each side's snapshot at the first step that
//! differs. This prints the step, the sections, and the two files to diff.
//! `just scenarios` leaves the transcript, so no game is launched.

use std::process::Command;

use crate::error::{self, ProcessError};

#[derive(clap::Args)]
pub struct Args {
    /// The scenario, by its file's name without `.txt`.
    name: String,
}

pub fn run(Args { name }: Args) -> Result<String, ProcessError> {
    let out = crate::workspace().join("target/scenario-diff");
    let output = error::output(
        Command::new("cargo")
            .current_dir(crate::workspace())
            .args([
                "run",
                "-q",
                "-p",
                "sts2-golden",
                "--example",
                "scenario_diff",
                "--",
            ])
            .arg(&name)
            .arg(&out),
    )?;
    let mut said = String::from_utf8_lossy(&output.stdout).into_owned();
    if !said.starts_with("the two sides agree") {
        let side = |end: &str| out.join(format!("{name}.{end}")).display().to_string();
        said.push_str(&format!("{}\n{}\n", side("sim"), side("game")));
    }
    Ok(said)
}

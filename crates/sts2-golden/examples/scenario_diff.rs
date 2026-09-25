//! Where one scenario's simulator and game part, for `cargo xtask scenario-diff`.
//!
//! ```text
//! scenario_diff <name> <out dir>
//! ```
//!
//! Walks the scenario on the simulator and on the game's cached transcript,
//! step by step. It stops at the first step whose sections or question
//! differ. Each side's whole snapshot goes to `<out dir>/<name>.sim` and
//! `.game`, and the step and the sections are printed. The cache is what `just scenarios`
//! left, so no game is launched.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct, Digested as _, Play, Snapshot};

type V = V0_107_1;

/// Find the first step a scenario's two sides disagree on.
#[derive(Parser)]
struct Cli {
    /// The scenario, by its file's name without `.txt`.
    name: String,
    /// Where the two snapshots are written.
    out: PathBuf,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("reading the suite: {0}")]
    Suite(String),
    #[error("no scenario is named {0}")]
    NoSuchScenario(String),
    #[error("reading {name}: {source}")]
    Recording {
        name: String,
        source: sts2_headless::recording::Error,
    },
    #[error("starting {name}: {source}")]
    Begin {
        name: String,
        source: sts2_sim::Error,
    },
    #[error("{side} at step {at}: {message}")]
    Step {
        side: &'static str,
        at: usize,
        message: String,
    },
    #[error("writing {}: {source}", path.display())]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// One step on one side, and what it leaves showing.
fn step<C: Construct<V>>(
    side: &'static str,
    at: usize,
    driven: &mut C,
    step: &sts2_core::Step<V>,
) -> Result<Snapshot<V>, Error>
where
    C::Error: std::fmt::Display,
{
    let failed = |message: String| Error::Step { side, at, message };
    driven
        .step(step)
        .map_err(|e| failed(e.to_string()))?
        .map_err(|refusal| failed(format!("refused: {refusal}")))?;
    Play::<V>::observe(driven).map_err(|e| failed(e.to_string()))
}

fn run(Cli { name, out }: Cli) -> Result<(), Error> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios");
    let written = sts2_golden::suite::read_suite(&dir).map_err(Error::Suite)?;
    let entry = written
        .iter()
        .find(|entry| entry.name == name)
        .ok_or_else(|| Error::NoSuchScenario(name.clone()))?;
    let stamp = serde_json::to_string(&sts2_headless::Stamp::of(
        &<V as sts2_core::ContentVersion>::VERSION,
    ))
    .expect("a stamp is json");
    let recording = format!("{stamp}\n{}", entry.commands);
    let recorded = sts2_headless::recording::read::<V>(std::io::Cursor::new(recording.as_bytes()))
        .map_err(|source| Error::Recording {
            name: name.clone(),
            source,
        })?;

    let mut sim =
        sts2_sim::Sim::<V>::begin((), recorded.start()).map_err(|source| Error::Begin {
            name: name.clone(),
            source,
        })?;
    let mut game = sts2_golden::oracle_or_explain::<V>(&name, recorded.start());
    for (at, taken) in recorded.steps().iter().enumerate() {
        let at = at + 1;
        let simulated = step("the simulator", at, &mut sim, taken)?;
        let played = step("the game", at, &mut game, taken)?;
        let sections: Vec<_> = simulated
            .observation
            .sections()
            .iter()
            .zip(played.observation.sections().iter())
            .filter(|(a, b)| a != b)
            .map(|(a, _)| format!("{:?}", a.0))
            .collect();
        let asked = sts2_core::Digest::of_question(simulated.asking.as_ref())
            != sts2_core::Digest::of_question(played.asking.as_ref());
        if sections.is_empty() && !asked {
            continue;
        }
        std::fs::create_dir_all(&out).map_err(|source| Error::Write {
            path: out.clone(),
            source,
        })?;
        for (end, snapshot) in [("sim", &simulated), ("game", &played)] {
            let path = out.join(format!("{name}.{end}"));
            std::fs::write(&path, format!("{snapshot:#?}\n"))
                .map_err(|source| Error::Write { path, source })?;
        }
        let question = if asked { " and the question" } else { "" };
        println!("step {at} differs in {}{question}", sections.join(", "));
        return Ok(());
    }
    println!("the two sides agree at every step");
    Ok(())
}

//! How far the simulator follows one recorded run, for `cargo xtask record`.
//!
//! ```text
//! follow_run <file> <index>
//! ```
//!
//! Prints `whole <steps>` when every position agreed, or `parted <step>
//! <steps>` at the first that did not. An unported path panics, and the
//! caller reads the panic off stderr, so this runs in a process of its own.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct as _, Gift, Play as _, Seed, Setup, Start};
use sts2_golden::recorded::{Followed, Recorded, follow};
use sts2_sim::Sim;

#[path = "carried/mod.rs"]
mod carried;
use carried::CARRIED;

type V = V0_107_1;

/// Replay one recorded run on the simulator.
#[derive(Parser)]
struct Cli {
    /// The file of recorded runs.
    file: PathBuf,
    /// Which run in it, counted from zero.
    index: usize,
}

fn main() -> ExitCode {
    let Cli { file, index } = Cli::parse();
    let bytes = std::fs::read(&file).unwrap_or_else(|e| panic!("reading {}: {e}", file.display()));
    let recorded = Recorded::decode(&bytes).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let run = &recorded.runs[index];
    let start = Start::<V> {
        seed: Seed::new(&run.seed).expect("a recorded seed is a seed"),
        character: from_id(&run.character),
        ascension: from_id(&run.ascension),
        modifiers: Vec::new(),
        wins: run.wins,
    };
    let mut sim = Sim::<V>::begin((), &start).expect("the run starts");
    for (relic, count) in CARRIED {
        for _ in 0..count {
            sim.setup(&Setup::Grant(Gift::Relic(relic)))
                .expect("a grant applies")
                .expect("and is not refused");
        }
    }
    match follow(run, &mut sim) {
        Followed::Whole => println!("whole {}", run.moves.len()),
        Followed::PartedAt(step) => println!("parted {step} {}", run.moves.len()),
    }
    ExitCode::SUCCESS
}

/// A value by the id the game spells it with.
fn from_id<T: serde::de::DeserializeOwned>(id: &str) -> T {
    serde_json::from_value(serde_json::Value::String(id.to_owned()))
        .unwrap_or_else(|e| panic!("{id:?}: {e}"))
}

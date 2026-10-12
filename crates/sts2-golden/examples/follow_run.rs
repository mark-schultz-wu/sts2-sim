//! How far the simulator follows one recorded run, for `cargo xtask record`.
//!
//! ```text
//! follow_run <file> [index]
//! ```
//!
//! Prints `<character> whole <steps>` when every position agreed, or
//! `<character> parted <step> <steps> <after>` at the first that did not,
//! where `after` names the move that led there. Without an index it prints
//! how many runs the file holds. An unported path panics, and the
//! caller reads the panic off stderr, so this runs in a process of its own.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct as _, Gift, Play as _, Setup};
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
    index: Option<usize>,
}

fn main() -> ExitCode {
    let Cli { file, index } = Cli::parse();
    let bytes = std::fs::read(&file).unwrap_or_else(|e| panic!("reading {}: {e}", file.display()));
    let recorded = Recorded::decode(&bytes).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let Some(index) = index else {
        println!("{}", recorded.runs.len());
        return ExitCode::SUCCESS;
    };
    let run = &recorded.runs[index];
    // First, so a panic below still says whose run it was.
    print!("{} ", run.character);
    std::io::Write::flush(&mut std::io::stdout()).expect("stdout takes a line");
    let mut sim = Sim::<V>::begin((), &run.start()).expect("the run starts");
    for (relic, count) in CARRIED {
        for _ in 0..count {
            sim.setup(&Setup::Grant(Gift::Relic(relic)))
                .expect("a grant applies")
                .expect("and is not refused");
        }
    }
    match follow(run, &mut sim) {
        Followed::Whole => println!("whole {}", run.digests.len()),
        Followed::PartedAt { step, after } => {
            println!("parted {step} {} {after}", run.digests.len());
        }
    }
    ExitCode::SUCCESS
}

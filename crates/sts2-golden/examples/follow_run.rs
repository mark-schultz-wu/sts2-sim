//! How far the simulator follows one recorded run, for `cargo xtask record`.
//!
//! ```text
//! follow_run <file> [index]
//! follow_run <file> <index> --scenario
//! ```
//!
//! Prints `<character> whole <steps>` when every position agreed, or
//! `<character> parted <step> <steps> <after>` at the first that did not,
//! where `after` names the move that led there. Without an index it prints
//! how many runs the file holds. An unported path panics, and the
//! caller reads the panic off stderr, so this runs in a process of its own.
//!
//! `--scenario` prints the run instead, as a scenario's commands, up to the
//! move that led to its first split. Up to there the simulator's list is the
//! game's, so its moves are the game's. `cargo xtask split` writes it out.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct as _, Gift, Play as _, Setup, Step};
use sts2_golden::recorded::{Followed, Recorded, Run, follow};
use sts2_headless::recording;
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
    /// Print the run as a scenario, up to its first split.
    #[arg(long, requires = "index")]
    scenario: bool,
}

fn main() -> ExitCode {
    let Cli {
        file,
        index,
        scenario,
    } = Cli::parse();
    let bytes = std::fs::read(&file).unwrap_or_else(|e| panic!("reading {}: {e}", file.display()));
    let recorded = Recorded::decode(&bytes).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let Some(index) = index else {
        println!("{}", recorded.runs.len());
        return ExitCode::SUCCESS;
    };
    let run = &recorded.runs[index];
    if scenario {
        print_scenario(run);
        return ExitCode::SUCCESS;
    }
    // First, so a panic below still says whose run it was.
    print!("{} ", run.character);
    std::io::Write::flush(&mut std::io::stdout()).expect("stdout takes a line");
    let mut sim = begun(run);
    match follow(run, &mut sim) {
        Followed::Whole => println!("whole {}", run.digests.len()),
        Followed::PartedAt { step, after } => {
            println!("parted {step} {} {after}", run.digests.len());
        }
    }
    ExitCode::SUCCESS
}

/// The simulator on `run`'s start, carrying what the run carried.
fn begun(run: &Run) -> Sim<V> {
    let mut sim = Sim::<V>::begin((), &run.start()).expect("the run starts");
    for (relic, count) in CARRIED {
        for _ in 0..count {
            sim.setup(&Setup::Grant(Gift::Relic(relic)))
                .expect("a grant applies")
                .expect("and is not refused");
        }
    }
    sim
}

/// `run` as a scenario's commands: its start, what it carried, and its moves
/// up to and including the one that led to its first split.
fn print_scenario(run: &Run) {
    let moves = match follow(run, &mut begun(run)) {
        Followed::Whole => run.moves.len(),
        Followed::PartedAt { step, .. } => step,
    };
    let mut out = std::io::stdout().lock();
    let wrote = "stdout takes the scenario";
    recording::began(&mut out, &run.start::<V>()).expect(wrote);
    for (relic, count) in CARRIED {
        for _ in 0..count {
            let grant = Step::<V>::Setup(Setup::Grant(Gift::Relic(relic)));
            recording::append(&mut out, &grant).expect(wrote);
        }
    }
    let mut sim = begun(run);
    for &at in &run.moves[..moves] {
        let seen = sim.observe().expect("the simulator observes");
        let decision = seen.observation.moves[usize::from(at)];
        recording::append(&mut out, &Step::Decision(decision)).expect(wrote);
        sim.play(&decision)
            .expect("the move plays")
            .expect("and is not refused");
    }
}

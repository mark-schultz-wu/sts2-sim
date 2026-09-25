//! Every allocation a replay makes, and which line made it.
//!
//! The `allocations` bench counts them per decision. This says where they come
//! from, by doing nothing but replay so that a heap profiler has one thing to
//! attribute.
//!
//! ```text
//! cargo build --release -p sts2-golden --example replay_sites
//! valgrind --tool=dhat --fullpath-after= --dhat-out-file=dhat.json \
//!     ./target/release/examples/replay_sites [rules]
//! ./tools/dhat-sites.py dhat.json --divide-by <REPLAYS × steps>
//! ```
//!
//! With `rules`, each step goes through `Sim::advance`, which is what a search
//! pays. Without it, through `Play::step`, which also builds the observation.

use std::process::ExitCode;

use clap::Parser;
use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct, Play, Step};
use sts2_golden::bench::reached;
use sts2_golden::{every_run, recording};
use sts2_sim::Sim;

type V = V0_107_1;

/// Enough replays that loading the recording is a rounding error.
const REPLAYS: usize = 200;

/// Replay the first recorded run, over and over.
#[derive(Parser)]
struct Cli {
    /// What each step goes through.
    #[arg(value_enum, default_value_t = Through::Play)]
    through: Through,
}

/// `Play::step`, which builds the observation, or `Sim::advance`, which is
/// what a search pays.
#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum Through {
    Play,
    Rules,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Golden(#[from] sts2_golden::Error),
    #[error("starting the run: {0}")]
    Begin(#[source] sts2_sim::Error),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Error> {
    let rules = cli.through == Through::Rules;
    let name = every_run()?.swap_remove(0);
    let recorded = recording::<V>(&name)?;
    let start = recorded.start().clone();
    let steps: Vec<Step<V>> = recorded.steps().to_vec();
    let supported = reached::<V, Sim<V>>(&start, &steps);
    let played = &steps[..supported];

    println!(
        "{name}: {REPLAYS} replays of {supported} steps, divide by {}",
        REPLAYS * supported
    );
    for _ in 0..REPLAYS {
        let mut sim = Sim::<V>::begin((), &start).map_err(Error::Begin)?;
        for step in played {
            if rules {
                let _ = std::hint::black_box(sim.advance(step));
            } else {
                let _ = std::hint::black_box(sim.step(step));
            }
        }
    }
    Ok(())
}

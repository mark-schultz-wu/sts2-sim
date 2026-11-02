//! Replay the recorded run once under the instructions layer, and write where
//! its instructions went as folded stacks.
//!
//! `cargo xtask trace record` builds this with the `profiling` profile and
//! runs it. The file it writes has one line per span path, root first. Each
//! line holds what that path's own span ran, and how many times it ran. Map generation is left out,
//! because a search inside a fight never generates a map.
//!
//! One replay runs untraced first. What the process does once, such as a
//! table built on first use, then lands on none of the replays traced.

mod layer;

use std::fmt::Write as _;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::Step;
use sts2_golden::bench::reached;
use sts2_golden::{every_run, recording};
use sts2_sim::Sim;
use tracing_subscriber::layer::SubscriberExt as _;

type V = V0_107_1;

/// The span map generation runs under.
const MAP: &str = "act_map";

/// Where map generation's own folded stacks go, beside `out`.
fn map_path(out: &str) -> String {
    match out.strip_suffix(".folded") {
        Some(stem) => format!("{stem}.map.folded"),
        None => format!("{out}.map"),
    }
}

/// Replays traced, so a per-replay cost such as starting the run is spread
/// as it is in the `instructions` replay.
const REPLAYS: usize = 20;

/// Replay the recorded run traced, and write where its instructions went.
#[derive(Parser)]
struct Cli {
    /// The folded file to write. Map generation's goes beside it.
    out: String,
}

/// Why no trace was written.
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Golden(#[from] sts2_golden::Error),
    #[error("{name} does not start a run: {source}")]
    Begin {
        name: String,
        source: sts2_sim::Error,
    },
    #[error("writing {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
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

fn run(Cli { out }: Cli) -> Result<(), Error> {
    let name = every_run()?.swap_remove(0);
    let recorded = recording::<V>(&name)?;
    let start = recorded.start().clone();
    let steps: Vec<Step<V>> = recorded.steps().to_vec();
    let supported = reached::<V>(&start, &steps);
    if let Err(source) = Sim::<V>::begin(&start) {
        return Err(Error::Begin { name, source });
    }
    let replay = || {
        let mut sim = Sim::<V>::begin(&start).expect("the run began once already");
        for step in &steps[..supported] {
            let _ = std::hint::black_box(sim.advance(step));
        }
    };
    // With no subscriber yet, every span is off.
    replay();

    tracing::subscriber::set_global_default(
        tracing_subscriber::registry().with(layer::Instructions),
    )
    .expect("nothing else has set a subscriber");
    let overhead = layer::calibrate();
    {
        let _replays = tracing::trace_span!("replay").entered();
        for _ in 0..REPLAYS {
            replay();
        }
    }
    let supported = supported * REPLAYS;

    let folded = layer::folded(overhead, MAP);
    let mut text = format!("# steps {supported} period 1\n");
    let mut total = 0;
    for (path, own, calls) in &folded.outside {
        writeln!(text, "{path} {own} {calls}").unwrap();
        total += own;
    }
    write(&out, text)?;
    // Map generation apart, counted per map built rather than per step.
    let mut text = format!("# steps {} period 1\n", folded.splits);
    for (path, own, calls) in &folded.under {
        writeln!(text, "{path} {own} {calls}").unwrap();
    }
    let maps = map_path(&out);
    write(&maps, text)?;
    println!(
        "{name}: {supported} steps, {total} instructions outside map generation, {} per step\n\
         map generation {} left out, and {} per span taken off its parent, \
         {} more for each span above that, {} off itself",
        total / supported as u64,
        layer::inclusive_of(MAP),
        overhead.parent,
        overhead.per_level,
        overhead.own,
    );
    Ok(())
}

fn write(path: &str, text: String) -> Result<(), Error> {
    std::fs::write(path, text).map_err(|source| Error::Write {
        path: path.into(),
        source,
    })
}

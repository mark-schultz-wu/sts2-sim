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

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Play as _, Step};
use sts2_golden::bench::reached;
use sts2_golden::{every_run, recording};
use sts2_sim::Sim;
use tracing_subscriber::layer::SubscriberExt as _;

type V = V0_107_1;

/// The span map generation runs under.
const MAP: &str = "act_map";

/// Replays traced, so a per-replay cost such as starting the run is spread
/// as it is in the `instructions` replay.
const REPLAYS: usize = 20;

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: sts2-trace <folded file to write>");

    let name = every_run()
        .into_iter()
        .next()
        .expect("a recorded run to replay");
    let recorded = recording::<V>(&name);
    let start = recorded.start().clone();
    let steps: Vec<Step<V>> = recorded.steps().to_vec();
    let supported = reached::<V, Sim<V>>(&start, &steps);
    let replay = || {
        let mut sim = Sim::<V>::begin((), &start).expect("a recording starts a run");
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

    let stacks = layer::folded(overhead, MAP);
    let mut text = format!("# steps {supported} period 1\n");
    let mut total = 0;
    for (path, own, calls) in &stacks {
        writeln!(text, "{path} {own} {calls}").unwrap();
        total += own;
    }
    std::fs::write(&out, text).unwrap_or_else(|e| panic!("writing {out}: {e}"));
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
}

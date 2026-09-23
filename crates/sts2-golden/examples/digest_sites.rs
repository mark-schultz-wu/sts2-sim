//! Every allocation a comparison's digest makes, and which line made it.
//!
//! The `allocations` bench counts them per decision. This says where they come
//! from. It digests the simulator's lent facts at every step of a replay and
//! does nothing else, so a heap profiler has one thing to attribute.
//!
//! ```text
//! cargo build --release -p sts2-golden --example digest_sites
//! valgrind --tool=dhat --fullpath-after= --dhat-out-file=dhat.json \
//!     ./target/release/examples/digest_sites
//! ./tools/dhat-sites.py dhat.json --divide-by <REPLAYS × steps>
//! ```

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Digested, Play, Step};
use sts2_golden::bench::reached;
use sts2_golden::{every_run, recording};
use sts2_sim::Sim;

type V = V0_107_1;

/// Digests taken at each step, so the replay that reaches them is a rounding
/// error.
const DIGESTS: usize = 200;

fn main() {
    let name = every_run()
        .into_iter()
        .next()
        .expect("a recorded run to replay");
    let recorded = recording::<V>(&name);
    let start = recorded.start().clone();
    let steps: Vec<Step<V>> = recorded.steps().to_vec();
    let supported = reached::<V, Sim<V>>(&start, &steps);

    println!(
        "{name}: {DIGESTS} digests at each of {supported} steps, divide by {}",
        DIGESTS * supported
    );
    let mut sim = Sim::<V>::begin((), &start).expect("a recording starts a run");
    for step in &steps[..supported] {
        let _ = sim.advance(step);
        for _ in 0..DIGESTS {
            std::hint::black_box(sim.observed().rules_digest());
        }
    }
}

//! Every allocation one branch of a run makes, and which list made it.
//!
//! The `decision` bench counts them. This says where they come from, by doing
//! nothing but clone so that a heap profiler has one thing to attribute.
//!
//! ```text
//! cargo build --release -p sts2-golden --example clone_sites
//! valgrind --tool=dhat --dhat-out-file=dhat.json \
//!     ./target/release/examples/clone_sites
//! ./tools/dhat-sites.py dhat.json
//! ```
//!
//! The position is the recording's last `PlayCard`, which is the one the
//! `decision` bench reports and the one a search spends its time in.

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::Step;
use sts2_golden::bench::{kind, reached, upto};
use sts2_golden::{every_run, recording};
use sts2_sim::Sim;

type V = V0_107_1;

/// Enough branches to drown the setup, few enough to profile in a minute.
///
/// Reaching the position replays five hundred steps, and each of those
/// allocates more than a branch does. So the loop has to be long enough that
/// the setup is a rounding error in the profile.
const BRANCHES: usize = 50_000;

fn main() {
    let name = every_run()
        .into_iter()
        .next()
        .expect("a recorded run to branch");
    let recorded = recording::<V>(&name);
    let start = recorded.start().clone();
    let steps: Vec<Step<V>> = recorded.steps().to_vec();
    let supported = reached::<V, Sim<V>>(&start, &steps);
    let at = (0..supported)
        .rfind(|&at| kind(&steps[at]) == "PlayCard")
        .expect("the recording plays a card");

    let base: Sim<V> = upto(&start, &steps, at);
    println!("{name} @{at}, {BRANCHES} branches");
    for _ in 0..BRANCHES {
        std::hint::black_box(base.clone());
    }
}

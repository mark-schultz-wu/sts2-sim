//! The simulator's legal moves, held to a run somebody actually played.
//!
//! `Sim::legal` reads the moves off the run, without building the report that
//! also lists them. A search picks from it, so a move it leaves out is a line
//! the search never considers. A recording is a run the game accepted. So every
//! move it makes was legal where it made it, and has to be on the list.

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct, Decision, Play, Step};
use sts2_golden::bench::reached;
use sts2_golden::{every_run, recording};
use sts2_sim::Sim;

type V = V0_107_1;

#[test]
fn every_move_a_recording_played_was_on_the_list() {
    for name in every_run() {
        let recorded = recording::<V>(&name);
        let start = recorded.start().clone();
        let steps: Vec<Step<V>> = recorded.steps().to_vec();
        let supported = reached::<V, Sim<V>>(&start, &steps);
        let mut sim = Sim::<V>::begin((), &start).expect("a recording starts a run");

        for (at, step) in steps[..supported].iter().enumerate() {
            // An answer is listed by size, as the first `n` options, not as
            // every selection. The recording's own choice need not be one.
            if let Step::Decision(decision) = step
                && !matches!(decision, Decision::Answer(_))
            {
                let legal = sim.legal();
                assert!(
                    legal.contains(decision),
                    "{name} @{at}: played {decision:?}, which is not among {legal:?}"
                );
            }
            sim.step(step)
                .expect("a supported step applies")
                .expect("and is not refused");
        }
    }
}

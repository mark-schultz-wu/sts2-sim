//! The simulator's lent facts, held to the report it builds from them.
//!
//! A comparison digests `Sim::observed` rather than build an observation. So
//! the lent facts have to be the reported ones, field for field. The digest of
//! each, and of each section, has to agree at every step of every recording.

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct, Digested, Play, Step};
use sts2_golden::bench::reached;
use sts2_golden::{every_run, recording};
use sts2_sim::Sim;

type V = V0_107_1;

#[test]
fn the_lent_facts_digest_as_the_report_does() {
    for name in every_run().expect("the recorded runs are readable") {
        let recorded = recording::<V>(&name).expect("a recorded run reads");
        let start = recorded.start().clone();
        let steps: Vec<Step<V>> = recorded.steps().to_vec();
        let supported = reached::<V, Sim<V>>(&start, &steps);
        let mut sim = Sim::<V>::begin((), &start).expect("a recording starts a run");

        for (at, step) in steps[..supported].iter().enumerate() {
            sim.step(step)
                .expect("a supported step applies")
                .expect("and is not refused");
            let reported = sim.observe().expect("the simulator reports").observation;
            let lent = sim.observed();

            assert_eq!(
                lent.rules_digest(),
                reported.rules_digest(),
                "{name} @{at}: the lent facts are not the reported ones",
            );
            assert_eq!(
                lent.digest(),
                reported.digest(),
                "{name} @{at}: prose included"
            );
            for ((section, lent), (_, reported)) in lent.sections().iter().zip(reported.sections())
            {
                assert_eq!(*lent, reported, "{name} @{at}: section {section}");
            }
        }
    }
}

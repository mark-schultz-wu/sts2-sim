//! A fight paused on a question clones, and each copy goes on by itself.
//!
//! A search branches where a player has to answer. So a simulator stopped
//! on any question clones into copies, and each takes its own answer and
//! runs on. `docs/ASYNC.md` requires it of the way the simulator pauses.

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::Step;
use sts2_golden::bench::reached;
use sts2_golden::{every_run, recording};
use sts2_sim::Sim;

type V = V0_107_1;

#[test]
fn a_clone_taken_at_a_question_answers_as_the_original_does() {
    let (mut paused, mut cloning) = (0usize, std::time::Duration::ZERO);
    for name in every_run().expect("the recorded runs are readable") {
        let recorded = recording::<V>(&name).expect("a recorded run reads");
        let start = recorded.start().clone();
        let steps: Vec<Step<V>> = recorded.steps().to_vec();
        let supported = reached::<V>(&start, &steps);
        let mut sim = Sim::<V>::begin(&start).expect("a recording starts a run");

        for (at, step) in steps[..supported].iter().enumerate() {
            let asked = sim.observe().expect("a run observes").asking.is_some();
            if asked && let Step::Decision(decision) = step {
                paused += 1;
                let begun = std::time::Instant::now();
                let mut copy = sim.clone();
                let mut other = sim.clone();
                cloning += begun.elapsed();
                // Another answer, on a copy of its own, leaves the others be.
                if let Some(elsewhere) = sim.legal().iter().find(|&legal| legal != decision) {
                    other
                        .play(elsewhere)
                        .expect("a legal answer applies")
                        .expect("and is not refused");
                }
                copy.play(decision)
                    .expect("the recorded answer applies to the copy")
                    .expect("and is not refused");
                sim.step(step)
                    .expect("a supported step applies")
                    .expect("and is not refused");
                assert_eq!(
                    copy.observe().expect("the copy observes"),
                    sim.observe().expect("the run observes"),
                    "{name} @{at}: the copy answered {decision:?} differently"
                );
                continue;
            }
            sim.step(step)
                .expect("a supported step applies")
                .expect("and is not refused");
        }
    }
    assert!(paused > 0, "no recorded run stopped on a question");
    eprintln!(
        "{paused} questions, each cloned twice, {:?} a clone",
        cloning / (2 * u32::try_from(paused).expect("a count of questions fits"))
    );
}

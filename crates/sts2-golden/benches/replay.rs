//! What a decision costs the simulator.
//!
//! The bar is `spikes/spike17-dod`, which runs a decision in 135-233 ns on this
//! hardware. That spike is a different shape: a borrowed fight and a cloned
//! `Combat` of four kilobytes. So this is not a like-for-like race. It is the
//! number that says how far apart the two are.
//!
//! No harness and no dependency, the way the spike measured itself. A median of
//! nine rounds, which was stable there to about a percent.
//!
//! Within one build. Two builds of the same code have measured ten percent
//! apart here, so a small move is not a result on its own. The allocation
//! counts beside each number do not wobble at all, which is why they are here:
//! a cut that removes a `Vec` shows up there whether or not the clock agrees.

use std::panic::{self, AssertUnwindSafe};
use std::time::Instant;

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct, Play, Start, Step};
use sts2_golden::{Counted, allocations, every_run, recording};
use sts2_sim::Sim;

type V = V0_107_1;

#[global_allocator]
static ALLOCATOR: Counted = Counted::system();

const ROUNDS: usize = 9;

fn main() {
    for name in every_run() {
        let recorded = recording::<V>(&name);
        let start = recorded.start().clone();
        let steps: Vec<Step<V>> = recorded.steps().to_vec();

        // Only the prefix the simulator manages. The rest measures nothing yet.
        let reached = reached(&start, &steps);
        let mut rounds: Vec<f64> = (0..ROUNDS)
            .map(|_| {
                let at = Instant::now();
                let done = played(&start, &steps[..reached]);
                assert_eq!(done, reached);
                at.elapsed().as_nanos() as f64 / reached as f64
            })
            .collect();
        rounds.sort_by(f64::total_cmp);

        // Counted separately from the timing. The counters are two relaxed adds
        // on the way through the allocator, so a run that reports them is not a
        // run whose clock is worth reading.
        let (_, whole) = allocations(|| {
            played(&start, &steps[..reached]);
        });

        println!(
            "{name}: {reached}/{} steps, {:.0} ns per step (median of {ROUNDS}), \
             {:.1} allocations per step, {:.0} bytes",
            steps.len(),
            rounds[ROUNDS / 2],
            whole.calls as f64 / reached as f64,
            whole.bytes as f64 / reached as f64,
        );

        // Per step, because one of them generates a whole act map and the
        // average of five hundred hides which. Two numbers each: the decision
        // on its own, and the decision with a report asked for afterwards. A
        // caller that compares two implementations pays the second; one that
        // searches pays the first.
        for at in 0..reached {
            let time = |take: &mut dyn FnMut(&mut Sim<V>)| {
                let mut rounds: Vec<f64> = (0..ROUNDS)
                    .map(|_| {
                        let mut sim = upto(&start, &steps, at);
                        let started = Instant::now();
                        take(&mut sim);
                        started.elapsed().as_nanos() as f64
                    })
                    .collect();
                rounds.sort_by(f64::total_cmp);
                rounds[ROUNDS / 2]
            };

            let step = &steps[at];
            let decided = time(&mut |sim| {
                std::hint::black_box(sim.step(step).ok());
            });
            let reported = time(&mut |sim| {
                let _ = sim.step(step);
                std::hint::black_box(sim.observe().ok());
            });

            let mut sim = upto(&start, &steps, at);
            let (_, asked) = allocations(|| {
                let _ = sim.step(step);
            });

            println!(
                "  step {at}: {decided:>9.0} ns  {}  ({reported:.0} ns reported, \
                 {} allocations, {} bytes)",
                named(step),
                asked.calls,
                asked.bytes,
            );
        }

        // What one observation costs to hand back, which a search pays at every
        // node whether it reads the observation or not.
        let mut sim = upto(&start, &steps, reached);
        let laps = 10_000;
        let at = Instant::now();
        for _ in 0..laps {
            std::hint::black_box(sim.observe().ok());
        }
        let elapsed = at.elapsed();
        let (_, asked) = allocations(|| {
            std::hint::black_box(sim.observe().ok());
        });
        println!(
            "  observe(): {:.0} ns, {} allocations, {} bytes",
            elapsed.as_nanos() as f64 / f64::from(laps),
            asked.calls,
            asked.bytes,
        );
    }
}

/// What a step is, for a report that has to name one.
fn named(step: &Step<V>) -> String {
    match step {
        Step::Decision(decision) => format!("{decision:?}")
            .split_whitespace()
            .next()
            .unwrap_or("?")
            .to_string(),
        Step::Setup(_) => "setup".to_string(),
    }
}

/// Replays a prefix known to be supported.
fn played(start: &Start<V>, steps: &[Step<V>]) -> usize {
    let mut sim = Sim::<V>::begin((), start).expect("a recording starts a run");
    for step in steps {
        assert!(matches!(sim.step(step), Ok(Ok(_))));
    }
    steps.len()
}

/// How much of a recording the simulator supports. Unported behavior panics
/// deliberately, so discover the boundary outside the timed region.
fn reached(start: &Start<V>, steps: &[Step<V>]) -> usize {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let begun = panic::catch_unwind(AssertUnwindSafe(|| Sim::<V>::begin((), start)));
    let Ok(Ok(mut sim)) = begun else {
        panic::set_hook(hook);
        return 0;
    };
    let reached = steps
        .iter()
        .position(|step| {
            !matches!(
                panic::catch_unwind(AssertUnwindSafe(|| sim.step(step))),
                Ok(Ok(Ok(_)))
            )
        })
        .unwrap_or(steps.len());
    panic::set_hook(hook);
    reached
}

/// A simulator standing where the first `count` steps leave the run.
fn upto(start: &Start<V>, steps: &[Step<V>], count: usize) -> Sim<V> {
    let mut sim = Sim::<V>::begin((), start).expect("a recording starts a run");
    for step in steps.iter().take(count) {
        let _ = sim.step(step);
    }
    sim
}

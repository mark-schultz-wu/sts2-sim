//! What one decision is made of.
//!
//! `replay` says what a step costs. This says where that cost goes.
//!
//! A search node is a clone and a step. So a decision splits:
//!
//! ```text
//!   clone    the run a node branches from
//!   command  the rules the decision runs
//!   report   the observation a caller that wants one derives
//! ```
//!
//! The clone and the report are timed on their own, the report by asking the
//! run that stands after the step for another one. The command is what is
//! left of the total once those two come off.
//!
//! `total` is a step through `Play`, which reports. `search` is a clone and a
//! step through `Sim::advance`, which does not. The gap is what a search
//! stopped paying. That column races `spikes/spike17-dod`, at 135-233 ns for
//! a decision and about 60 to clone its whole fight.
//!
//! One decision of each kind, the last the recording plays. A median of nine
//! rounds, with the allocations counted in a pass of their own.

use std::time::Instant;

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct, Play, Step};
use sts2_golden::bench::{kind, reached, upto};
use sts2_golden::{Counted, allocations, every_run, recording};
use sts2_sim::Sim;

type V = V0_107_1;

#[global_allocator]
static ALLOCATOR: Counted = Counted::system();

const ROUNDS: usize = 9;
const LAPS: u32 = 10_000;

fn main() {
    for name in every_run() {
        let recorded = recording::<V>(&name);
        let start = recorded.start().clone();
        let steps: Vec<Step<V>> = recorded.steps().to_vec();
        let supported = reached::<V, Sim<V>>(&start, &steps);

        println!(
            "{name}, nanoseconds and allocations per decision\n  \
             {:<18} {:>7} {:>8} {:>7} {:>7} {:>7}    {:>5} {:>7} {:>6} {:>6}",
            "",
            "clone",
            "command",
            "report",
            "total",
            "search",
            "clone",
            "command",
            "report",
            "search"
        );
        for at in last_of_each_kind(&steps[..supported]) {
            let step = &steps[at];
            let base: Sim<V> = upto(&start, &steps, at);

            let whole = time(|| {
                let mut sim = base.clone();
                std::hint::black_box(sim.step(step).ok());
            });
            let cloned = time(|| {
                std::hint::black_box(base.clone());
            });
            // The run as the step leaves it, which is what the command
            // reported from.
            let mut after = base.clone();
            let _ = after.step(step);
            let reported = time(|| {
                std::hint::black_box(after.observe().ok());
            });

            // A search node: the clone and the step, with no observation
            // derived off either end.
            let searched = time(|| {
                let mut sim = base.clone();
                std::hint::black_box(sim.advance(step).ok());
            });

            let (_, whole_calls) = allocations(|| {
                let mut sim = base.clone();
                let _ = sim.step(step);
            });
            let (_, clone_calls) = allocations(|| {
                std::hint::black_box(base.clone());
            });
            let (_, report_calls) = allocations(|| {
                std::hint::black_box(after.observe().ok());
            });
            let (_, search_calls) = allocations(|| {
                let mut sim = base.clone();
                let _ = sim.advance(step);
            });

            println!(
                "  {:<18} {:>7.0} {:>8.0} {:>7.0} {:>7.0} {:>7.0}    {:>5} {:>7} {:>6} {:>6}",
                format!("{} @{at}", kind(step)),
                cloned,
                whole - cloned - reported,
                reported,
                whole,
                searched,
                clone_calls.calls,
                whole_calls.calls - clone_calls.calls - report_calls.calls,
                report_calls.calls,
                search_calls.calls,
            );
        }
    }
}

/// The last step of each kind, in the order the recording first plays them.
///
/// The last rather than the first. An opening hand is the cheapest position a
/// fight has, and a search spends its time in the middle of one.
fn last_of_each_kind(steps: &[Step<V>]) -> Vec<usize> {
    let mut last: Vec<(String, usize)> = Vec::new();
    for (at, step) in steps.iter().enumerate() {
        let kind = kind(step);
        match last.iter_mut().find(|(seen, _)| *seen == kind) {
            Some((_, was)) => *was = at,
            None => last.push((kind, at)),
        }
    }
    last.into_iter().map(|(_, at)| at).collect()
}

/// A median of [`ROUNDS`] rounds, each of [`LAPS`] laps, in nanoseconds.
///
/// Laps because the parts of a decision are hundreds of nanoseconds. Reading
/// the clock twice costs enough at that size to be worth amortising.
fn time(mut take: impl FnMut()) -> f64 {
    let mut rounds: Vec<f64> = (0..ROUNDS)
        .map(|_| {
            let began = Instant::now();
            for _ in 0..LAPS {
                take();
            }
            began.elapsed().as_nanos() as f64 / f64::from(LAPS)
        })
        .collect();
    rounds.sort_by(f64::total_cmp);
    rounds[ROUNDS / 2]
}

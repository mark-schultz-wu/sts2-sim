//! What a decision asks the allocator for.
//!
//! The other two benches read the clock. This one counts calls. They are apart
//! because one binary cannot do both: a counting allocator takes in
//! criterion's own working memory, and criterion then samples a counter.
//!
//! Counting is the measurement worth trusting here. Two builds of the same
//! code measure ten percent apart on this machine, so the clock cannot see a
//! change worth less than that. The counts do not wobble at all, and a cut
//! that removes a `Vec` shows up whether or not the clock agrees.
//!
//! Three tables:
//!
//! ```text
//!   per decision kind   clone, command, report, the node a search pays, and
//!                       the legal moves it picks from
//!   the whole replay    calls and bytes per step
//!   per step            which steps ask for the most
//! ```

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct, Play, Start, Step};
use sts2_golden::bench::{kind, named, reached, upto};
use sts2_golden::{Counted, allocations, every_run, recording};
use sts2_sim::Sim;

type V = V0_107_1;

#[global_allocator]
static ALLOCATOR: Counted = Counted::system();

fn main() {
    for name in every_run() {
        let recorded = recording::<V>(&name);
        let start = recorded.start().clone();
        let steps: Vec<Step<V>> = recorded.steps().to_vec();
        let supported = reached::<V, Sim<V>>(&start, &steps);

        kinds(&name, &start, &steps, supported);
        whole(&name, &start, &steps, supported);
        per_step(&start, &steps, supported);
    }
}

/// One decision of each kind, the last the recording plays.
///
/// The last rather than the first. An opening hand is the cheapest position a
/// fight has, and a search spends its time in the middle of one.
fn kinds(name: &str, start: &Start<V>, steps: &[Step<V>], supported: usize) {
    println!(
        "{name}, allocations per decision\n  \
         {:<18} {:>6} {:>8} {:>7} {:>6} {:>6}",
        "", "clone", "command", "report", "node", "legal"
    );
    for at in last_of_each_kind(&steps[..supported]) {
        let step = &steps[at];
        let base: Sim<V> = upto(start, steps, at);
        // The run as the step leaves it, which is what a report reports from.
        let mut after = base.clone();
        let _ = after.step(step);

        let (_, cloned) = allocations(|| {
            std::hint::black_box(base.clone());
        });
        let (_, node) = allocations(|| {
            let mut sim = base.clone();
            let _ = sim.advance(step);
        });
        let (_, reported) = allocations(|| {
            std::hint::black_box(after.observe().ok());
        });
        let (_, offered) = allocations(|| {
            std::hint::black_box(base.legal());
        });

        println!(
            "  {:<18} {:>6} {:>8} {:>7} {:>6} {:>6}",
            format!("{} @{at}", kind(step)),
            cloned.calls,
            node.calls - cloned.calls,
            reported.calls,
            node.calls,
            offered.calls,
        );
    }
}

/// The recording's supported prefix, as one figure.
fn whole(name: &str, start: &Start<V>, steps: &[Step<V>], supported: usize) {
    let (_, asked) = allocations(|| {
        played(start, &steps[..supported]);
    });
    println!(
        "\n{name}: {supported}/{} steps, {:.1} allocations per step, {:.0} bytes\n",
        steps.len(),
        asked.calls as f64 / supported as f64,
        asked.bytes as f64 / supported as f64,
    );
}

/// Every step on its own.
///
/// Per step, because one of them generates a whole act map and the average of
/// five hundred hides which.
fn per_step(start: &Start<V>, steps: &[Step<V>], supported: usize) {
    for at in 0..supported {
        let step = &steps[at];
        let mut sim: Sim<V> = upto(start, steps, at);
        let (_, asked) = allocations(|| {
            let _ = sim.step(step);
        });
        println!(
            "  step {at}: {:>5} allocations, {:>7} bytes  {}",
            asked.calls,
            asked.bytes,
            named(step),
        );
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

/// The last step of each kind, in the order the recording first plays them.
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

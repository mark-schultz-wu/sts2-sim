//! What one decision is made of.
//!
//! `replay` says what a step costs. This says where that cost goes.
//!
//! A search node is a clone and a step, picked from the legal moves. So a
//! decision splits five ways, each timed on its own and not as a remainder:
//!
//! ```text
//!   clone    the run a node branches from
//!   command  the rules the decision runs, off a clone made beforehand
//!   report   the observation a caller that wants one derives
//!   digest   what a comparison takes instead: the rules digest of the lent facts
//!   node     the two a search pays, which is a clone and a command
//!   legal    the moves a search picks from, asked where the decision is made
//! ```
//!
//! `command` is the one criterion makes honest. Its clone is built in the
//! setup of each batch, so the number is the rules alone. It used to be what a
//! total had left once the other two came off.
//!
//! The bar is the deleted `spikes/spike17-dod`: 135-233 ns a decision, about
//! 60 to clone its fight. `node` races it. To compare two revisions, save a
//! baseline on each and let criterion do the statistics:
//!
//! ```text
//! git checkout main
//! cargo bench -p sts2-golden --bench decision -- --save-baseline main
//! git checkout -
//! cargo bench -p sts2-golden --bench decision -- --baseline main
//! ```
//!
//! The allocation counts live in `allocations`, because a counting allocator
//! and a sampling harness cannot share a binary.

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::Digested;
use sts2_core::{Construct, Play, Step};
use sts2_golden::bench::{kind, reached, upto};
use sts2_golden::{every_run, recording};
use sts2_sim::Sim;

type V = V0_107_1;

fn decisions(c: &mut Criterion) {
    for name in every_run().expect("the recorded runs are readable") {
        let recorded = recording::<V>(&name).expect("a recorded run reads");
        let start = recorded.start().clone();
        let steps: Vec<Step<V>> = recorded.steps().to_vec();
        let supported = reached::<V, Sim<V>>(&start, &steps);

        for at in last_of_each_kind(&steps[..supported]) {
            let step = &steps[at];
            let base: Sim<V> = upto(&start, &steps, at);
            // The run as the step leaves it, which is what a report reports
            // from.
            let mut after = base.clone();
            let _ = after.step(step);

            let mut group = c.benchmark_group(format!("{}@{at}", kind(step)));

            group.bench_function("clone", |b| b.iter(|| base.clone()));

            group.bench_function("command", |b| {
                b.iter_batched_ref(
                    || base.clone(),
                    |sim| sim.advance(step).ok(),
                    BatchSize::SmallInput,
                )
            });

            group.bench_function("report", |b| b.iter(|| after.observe().ok()));

            group.bench_function("digest", |b| b.iter(|| after.observed().rules_digest()));

            group.bench_function("legal", |b| b.iter(|| base.legal()));

            group.bench_function("node", |b| {
                b.iter(|| {
                    let mut sim = base.clone();
                    sim.advance(step).ok()
                })
            });

            group.bench_function("clone_from", |b| {
                let mut node = base.clone();
                b.iter(|| node.clone_from(&base))
            });

            group.bench_function("node_from", |b| {
                let mut node = base.clone();
                b.iter(|| {
                    node.clone_from(&base);
                    node.advance(step).ok()
                })
            });

            group.finish();
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

criterion_group!(benches, decisions);
criterion_main!(benches);

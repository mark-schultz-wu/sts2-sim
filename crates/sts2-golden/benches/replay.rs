//! What a whole recorded run costs to replay.
//!
//! One number: the recording's supported prefix, played from the start. It is
//! the end-to-end figure the per-decision bench takes apart, and the one that
//! moves when a change touches something the table does not name.
//!
//! Within one build. Two builds of the same code have measured ten percent
//! apart here, so a small move is not a result on its own. Criterion compares
//! two revisions properly, given a baseline saved on each:
//!
//! ```text
//! git checkout main
//! cargo bench -p sts2-golden --bench replay -- --save-baseline main
//! git checkout -
//! cargo bench -p sts2-golden --bench replay -- --baseline main
//! ```
//!
//! The allocation counts live in `allocations`, which also lists them per
//! step. A cut that removes a `Vec` shows up there whether or not the clock
//! agrees, which is why both benches exist.

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct, Play, Start, Step};
use sts2_golden::bench::{reached, upto};
use sts2_golden::{every_run, recording};
use sts2_sim::Sim;

type V = V0_107_1;

fn replays(c: &mut Criterion) {
    for name in every_run() {
        let recorded = recording::<V>(&name);
        let start = recorded.start().clone();
        let steps: Vec<Step<V>> = recorded.steps().to_vec();

        // Only the prefix the simulator manages. The rest measures nothing yet.
        let supported = reached::<V, Sim<V>>(&start, &steps);
        let played_steps = &steps[..supported];

        let mut group = c.benchmark_group(name);
        // So criterion reports the per-step figure beside the whole.
        group.throughput(Throughput::Elements(supported as u64));
        group.bench_function("replay", |b| {
            b.iter(|| played(&start, played_steps));
        });

        // What one observation costs to hand back. A caller comparing two
        // implementations pays this on every step; a search pays none of it.
        let mut standing: Sim<V> = upto(&start, &steps, supported);
        group.bench_function("observe", |b| b.iter(|| standing.observe().ok()));
        group.finish();
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

criterion_group!(benches, replays);
criterion_main!(benches);

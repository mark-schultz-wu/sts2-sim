//! Where a whole replay's time goes, by the kind of decision that spent it.
//!
//! The `decision` bench times one step of each kind, and picks the last one a
//! recording plays. That is the dearest position a kind reaches, so counting
//! kinds and multiplying overstates the total. This times every step where it
//! falls, and adds them up.
//!
//! Two walks of the same recording, neither of which branches it:
//!
//! ```text
//!   total    through `Play`, which runs the rules and derives an observation
//!   rules    through `Sim::advance`, which runs the rules alone
//! ```
//!
//! So the difference between them is what the observations cost. A search
//! pays the second column and a clone. A divergence run pays the first.
//!
//! The lowest of [`ROUNDS`] walks for each step, because a step runs once and
//! the interruptions only ever add.

use std::time::{Duration, Instant};

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct, Play, Step};
use sts2_golden::bench::{kind, reached};
use sts2_golden::{every_run, recording};
use sts2_sim::Sim;

type V = V0_107_1;

/// How many times each step is walked.
const ROUNDS: usize = 40;

fn main() {
    for name in every_run() {
        let recorded = recording::<V>(&name);
        let start = recorded.start().clone();
        let steps: Vec<Step<V>> = recorded.steps().to_vec();
        let supported = reached::<V, Sim<V>>(&start, &steps);
        let played = &steps[..supported];

        let reported = walk(&start, played, |sim, step| {
            let _ = sim.step(step);
        });
        let searched = walk(&start, played, |sim, step| {
            let _ = sim.advance(step);
        });

        let total: Duration = reported.iter().sum();
        let mut kinds: Vec<(String, usize, Duration, Duration)> = Vec::new();
        for (at, step) in played.iter().enumerate() {
            let named = kind(step);
            match kinds.iter_mut().find(|(seen, ..)| *seen == named) {
                Some((_, count, with, without)) => {
                    *count += 1;
                    *with += reported[at];
                    *without += searched[at];
                }
                None => kinds.push((named, 1, reported[at], searched[at])),
            }
        }
        kinds.sort_by_key(|(.., with, _)| std::cmp::Reverse(*with));

        println!(
            "{name}: {supported} steps in {:.3} ms, replayed through `Play`\n",
            total.as_secs_f64() * 1e3,
        );
        println!(
            "  {:<14} {:>6} {:>11} {:>7} {:>11} {:>10}",
            "decision", "count", "total", "share", "each", "rules",
        );
        for (named, count, with, without) in &kinds {
            println!(
                "  {:<14} {:>6} {:>9.3} ms {:>6.1}% {:>9.2} us {:>8.3} ms",
                named,
                count,
                with.as_secs_f64() * 1e3,
                with.as_secs_f64() / total.as_secs_f64() * 100.0,
                with.as_secs_f64() * 1e6 / *count as f64,
                without.as_secs_f64() * 1e3,
            );
        }
        let searched: Duration = searched.iter().sum();
        println!(
            "\n  the rules alone are {:.3} ms of it, so the observations are \
             {:.3} ms, or {:.0}%",
            searched.as_secs_f64() * 1e3,
            (total - searched).as_secs_f64() * 1e3,
            (total - searched).as_secs_f64() / total.as_secs_f64() * 100.0,
        );
    }
}

/// The lowest time each step took over [`ROUNDS`] walks of the recording.
fn walk(
    start: &sts2_core::Start<V>,
    steps: &[Step<V>],
    mut take: impl FnMut(&mut Sim<V>, &Step<V>),
) -> Vec<Duration> {
    let mut best = vec![Duration::MAX; steps.len()];
    for _ in 0..ROUNDS {
        let mut sim = Sim::<V>::begin((), start).expect("a recording starts a run");
        for (at, step) in steps.iter().enumerate() {
            let began = Instant::now();
            take(&mut sim, step);
            best[at] = best[at].min(began.elapsed());
        }
    }
    best
}

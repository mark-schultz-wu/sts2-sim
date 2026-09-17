//! Driving a recording into an implementation, for the benches to measure.
//!
//! The two benches here measure the same runs at different grain. `replay`
//! takes every step of a recording, and `decision` takes one decision apart.
//! Both have to put a run where a step happens before they can time it, and
//! that is what this is.
//!
//! Generic over the implementation rather than over the simulator. This crate
//! cannot name one. `sts2-sim` is a dev-dependency, and a library module
//! cannot reach a dev-dependency.

use std::panic::{self, AssertUnwindSafe};

use sts2_core::{Construct, ContentVersion, Start, Step};

/// What a step is, for a report that has to name one.
pub fn named<V: ContentVersion>(step: &Step<V>) -> String {
    match step {
        Step::Decision(decision) => format!("{decision:?}")
            .split_whitespace()
            .next()
            .unwrap_or("?")
            .to_string(),
        Step::Setup(_) => "setup".to_string(),
    }
}

/// Which decision a step is, without what it carries.
///
/// [`named`] identifies one step of a recording, so it keeps the payload. This
/// is for a report with a row per kind rather than a row per step.
pub fn kind<V: ContentVersion>(step: &Step<V>) -> String {
    let named = named(step);
    match named.split_once('(') {
        Some((variant, _)) => variant.to_string(),
        None => named,
    }
}

/// How much of a recording an implementation supports.
///
/// Unported behaviour panics deliberately, so the boundary is discovered here
/// rather than inside a timed region.
pub fn reached<V: ContentVersion, P: Construct<V, Host = ()>>(
    start: &Start<V>,
    steps: &[Step<V>],
) -> usize {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let begun = panic::catch_unwind(AssertUnwindSafe(|| P::begin((), start)));
    let Ok(Ok(mut playing)) = begun else {
        panic::set_hook(hook);
        return 0;
    };
    let reached = steps
        .iter()
        .position(|step| {
            !matches!(
                panic::catch_unwind(AssertUnwindSafe(|| playing.step(step))),
                Ok(Ok(Ok(_)))
            )
        })
        .unwrap_or(steps.len());
    panic::set_hook(hook);
    reached
}

/// A run standing where the first `count` steps leave it.
pub fn upto<V: ContentVersion, P: Construct<V, Host = ()>>(
    start: &Start<V>,
    steps: &[Step<V>],
    count: usize,
) -> P
where
    P::Error: std::fmt::Debug,
{
    let mut playing = P::begin((), start).expect("a recording starts a run");
    for step in steps.iter().take(count) {
        let _ = playing.step(step);
    }
    playing
}

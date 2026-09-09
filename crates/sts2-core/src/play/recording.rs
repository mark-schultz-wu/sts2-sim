//! A run, written down so it can be run again.
//!
//! A run is a seed and a sequence of decisions, and the build is deterministic,
//! so replaying the decisions reproduces the run. A recording holds no state.
//! It is the input to the divergence test: the same steps driven into the
//! oracle and into the simulator, compared step by step.
//!
//! [`Recording`] is parameterised by the build, and there is no version field.
//! `Recording<V>` *is* the recording of a `V` run.
//!
//! In-memory only for now. The crate README says why the format is this crate's
//! to choose, and where it lives until this crate chooses it.

use crate::ContentVersion;
use crate::Start;
use crate::Step;

/// A run of build `V`: what started it, and the steps that followed.
///
/// The start is a field rather than the first step. A recording that does not
/// begin with one is a fragment rather than a run. Holding it this way makes
/// that a thing the reader cannot produce, instead of a thing every caller has
/// to check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recording<V: ContentVersion> {
    start: Start<V>,
    steps: Vec<Step<V>>,
}

impl<V: ContentVersion> Recording<V> {
    pub fn new(start: Start<V>) -> Self {
        Self {
            start,
            steps: Vec::new(),
        }
    }

    /// What started the run this recorded.
    pub fn start(&self) -> &Start<V> {
        &self.start
    }

    pub fn steps(&self) -> &[Step<V>] {
        &self.steps
    }

    pub fn push(&mut self, step: impl Into<Step<V>>) {
        self.steps.push(step.into());
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }
}

impl<V: ContentVersion> Recording<V> {
    /// A recording of `start` and the steps that followed it.
    pub fn of(start: Start<V>, steps: impl IntoIterator<Item = Step<V>>) -> Self {
        Self {
            start,
            steps: steps.into_iter().collect(),
        }
    }
}

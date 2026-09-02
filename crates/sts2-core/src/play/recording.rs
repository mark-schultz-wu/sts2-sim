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
use crate::Step;

/// A run of build `V`, as the steps that made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recording<V: ContentVersion> {
    steps: Vec<Step<V>>,
}

impl<V: ContentVersion> Default for Recording<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: ContentVersion> Recording<V> {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
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

impl<V: ContentVersion> FromIterator<Step<V>> for Recording<V> {
    fn from_iter<I: IntoIterator<Item = Step<V>>>(steps: I) -> Self {
        Self {
            steps: steps.into_iter().collect(),
        }
    }
}

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
use crate::SeatIndex;
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
    /// The seat that took each step. As long as `steps`. A solo run's are all
    /// seat 0.
    seats: Vec<SeatIndex>,
}

impl<V: ContentVersion> Recording<V> {
    pub fn new(start: Start<V>) -> Self {
        Self {
            start,
            steps: Vec::new(),
            seats: Vec::new(),
        }
    }

    /// What started the run this recorded.
    pub fn start(&self) -> &Start<V> {
        &self.start
    }

    pub fn steps(&self) -> &[Step<V>] {
        &self.steps
    }

    /// The seat that took each step, in step order.
    pub fn seats(&self) -> &[SeatIndex] {
        &self.seats
    }

    /// Adds a step of a solo run.
    ///
    /// # Panics
    ///
    /// On a run of several players, where a step has to say whose it is.
    pub fn push(&mut self, step: impl Into<Step<V>>) {
        self.start.alone();
        self.push_by(SeatIndex(0), step);
    }

    /// Adds a step `seat` took.
    ///
    /// # Panics
    ///
    /// On a seat the run has no player for.
    pub fn push_by(&mut self, seat: SeatIndex, step: impl Into<Step<V>>) {
        assert!(
            seat.at() < self.start.players.len(),
            "seat {} in a run of {} players",
            seat.0,
            self.start.players.len()
        );
        self.steps.push(step.into());
        self.seats.push(seat);
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }
}

impl<V: ContentVersion> Recording<V> {
    /// A recording of a solo run: `start` and the steps that followed it.
    ///
    /// # Panics
    ///
    /// On a run of several players.
    pub fn of(start: Start<V>, steps: impl IntoIterator<Item = Step<V>>) -> Self {
        let mut recording = Self::new(start);
        for step in steps {
            recording.push(step);
        }
        recording
    }
}

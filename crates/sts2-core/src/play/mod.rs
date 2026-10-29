//! What any implementation of the game answers to: a [`RunTable`](table::RunTable),
//! the one trait every table implements, and what a move comes to.
//!
//! # Two results, and only one of them is a failure
//!
//! ```text
//! Result< Result< Decided, Refusal >, Self::Error >
//!         ^^^^^^^^^^^^^^^^^^^^^^^^^^ did the game accept it
//! ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ did the exchange happen
//! ```
//!
//! A [`Refusal`] is not an error. It is a legitimate, deterministic answer that
//! every implementation must give. The fuzzer's headline finding is the game
//! refusing a move the observation reported as legal.
//!
//! A *fault* is an error, and lives in the table's own error type. A socket
//! that died, a process that stopped answering, a reply that did not parse.

pub mod recording;
pub mod refusal;
pub mod seat;
pub mod table;
pub mod verdict;

use crate::{ContentVersion, Observation, Question, Refusal};

/// Where a run is standing.
///
/// The whole of what an implementation reports, and everything two of them are
/// compared on. The oracle's protocol envelope does not survive into here,
/// because none of it is a fact about the run. Not the sequence number, the
/// command name, or the game's own digest.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot<V: ContentVersion> {
    pub observation: Observation<V>,
    /// The question the game stopped to ask, for a decision that parked it.
    ///
    /// A parked game still reports where it is standing, so this is beside the
    /// observation rather than instead of it.
    pub asking: Option<Question<V>>,
}

/// What a decision produced, which is where it left the run and whether it
/// finished.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied<V: ContentVersion> {
    pub observation: Observation<V>,
    pub asking: Option<Question<V>>,
    /// False when combat ended mid-cascade and the decision was abandoned
    /// rather than completed.
    ///
    /// Not a failure. An implementation that resolved it fully has diverged,
    /// even where the state it arrived at looks agreeable.
    pub resolved: bool,
}

/// What a decision decided.
///
/// A fact about the decision and not about the run, which is why it is beside
/// [`Snapshot`] rather than inside it. A parked run's snapshot says nothing
/// about whether the last decision finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Decided {
    /// The decision ran to completion.
    Resolved,
    /// Combat ended mid-cascade and the decision was abandoned under it.
    ///
    /// Not a failure. An implementation that resolved it fully has diverged,
    /// even where the state it arrived at looks agreeable.
    Abandoned,
}

impl<V: ContentVersion> Snapshot<V> {
    /// This, beside what the decision that reached it decided.
    ///
    /// The inverse of [`Applied::split`]. An implementation that builds the
    /// two apart joins them here.
    pub fn applied(self, decided: Decided) -> Applied<V> {
        Applied {
            observation: self.observation,
            asking: self.asking,
            resolved: decided == Decided::Resolved,
        }
    }
}

impl<V: ContentVersion> Applied<V> {
    /// Where this left the run.
    pub fn snapshot(self) -> Snapshot<V> {
        Snapshot {
            observation: self.observation,
            asking: self.asking,
        }
    }

    /// What it decided, and where it left the run.
    pub fn split(self) -> (Decided, Snapshot<V>) {
        let decided = if self.resolved {
            Decided::Resolved
        } else {
            Decided::Abandoned
        };
        (decided, self.snapshot())
    }
}

/// Whether the exchange happened, and then whether the game accepted it.
pub type Answered<V> = Result<Applied<V>, Refusal<V>>;

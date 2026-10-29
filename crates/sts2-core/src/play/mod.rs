//! What any implementation of the game answers to.
//!
//! Two traits, split so that the real game cannot pick up methods no real game
//! has. The split also keeps a simulator from being forced to fake ones it has
//! not earned. Both are one method wide, and both hand back the same
//! [`Applied`]. That is the point: a comparison between two implementations is
//! a comparison of what they handed back.
//!
//! # Two results, and only one of them is a failure
//!
//! ```text
//! Result< Result< Applied<V>, Refusal >, Self::Error >
//!         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ did the game accept it
//! ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ did the exchange happen
//! ```
//!
//! A [`Refusal`] is not an error. It is a legitimate, deterministic answer that
//! both implementations must give. The fuzzer's headline finding is the game
//! refusing a move the observation reported as legal.
//!
//! A *fault* is an error, and lives in the implementation's own
//! [`Play::Error`]. A socket that died, a process that stopped answering, a
//! reply that did not parse.

pub mod recording;
pub mod refusal;
pub mod seat;
pub mod table;
pub mod verdict;

use crate::{ContentVersion, Decision, Observation, Question, Refusal, Setup, Start, Step};

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

/// Playing: every implementation, and nothing beyond what a run needs.
pub trait Play<V: ContentVersion>: Sized {
    /// A fault, which is not a refusal. See the module docs.
    type Error;

    /// What it takes to reach a game, before a run is started on it.
    ///
    /// A description rather than a resource. `()` for a simulator, a directory
    /// for something that owns a game process, and `()` again for something
    /// that reaches a game somebody else holds.
    type Host;

    /// Starts a run, and hands back the thing that plays it.
    ///
    /// The only way to get one, so a value of this type holds a run and "no
    /// run yet" stays out of every decision. One result, where every other
    /// method has two: this makes the run, so a game that will not start one
    /// leaves nothing to report from.
    fn begin(host: Self::Host, start: &Start<V>) -> Result<Self, Self::Error>;

    /// Plays one decision and reports what it produced.
    fn play(&mut self, decision: &Decision<V>) -> Result<Answered<V>, Self::Error>;

    /// Where the run is standing, without moving it.
    ///
    /// Not a [`Decision`], because it decides nothing. It is here for a caller
    /// that is a whole process per command with nothing in hand. Without it,
    /// such a front end reaches past the trait.
    ///
    /// Not refusable. [`Play::begin`] is the only way to hold one of these, so
    /// there is always a run to report on.
    fn observe(&mut self) -> Result<Snapshot<V>, Self::Error>;
}

/// Reaching a position that ordinary play takes an hour to arrive at.
///
/// Separate from [`Play`] because it is not a move in a run, and because not
/// every implementation can make one. The oracle has it because the mod does,
/// and a simulator gets it when it can.
pub trait Construct<V: ContentVersion>: Play<V> {
    /// Puts the run into a position, without playing to it.
    fn setup(&mut self, step: &Setup<V>) -> Result<Answered<V>, Self::Error>;

    /// Plays one line of a recording, whichever half it is.
    ///
    /// Replaying a recording into something that can construct is a fold over
    /// its steps. Something that cannot has to match on the [`Step`] itself and
    /// say so. That is the correct answer: a run that was constructed cannot be
    /// reproduced by an implementation that cannot construct.
    fn step(&mut self, step: &Step<V>) -> Result<Answered<V>, Self::Error> {
        match step {
            Step::Decision(decision) => self.play(decision),
            Step::Setup(setup) => self.setup(setup),
        }
    }
}

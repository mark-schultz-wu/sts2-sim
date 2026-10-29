//! What holds a run, every seat of it: the simulator or the real game.
//!
//! A run is played at a table. Each seat has a player, and the table takes
//! one seat's move at a time. Solo is a table of one seat. Nothing else
//! differs between solo and co-op here.
//!
//! The game is one machine for each seat, and the simulator holds one
//! machine's view for each seat. So a table speaks for every seat, and each
//! method names the seat it is about.
//!
//! `docs/RUNNERS.md` says how a runner drives one.

use crate::{
    ContentVersion, Decided, Decision, Observed, Refusal, SeatIndex, Setup, Snapshot, Start,
    Verdict,
};

/// What a move or a setup came to: what it decided, or the table's refusal.
pub type Settled<V> = Result<Decided, Refusal<V>>;

/// Everything that holds one run, for every seat of its lobby.
pub trait RunTable<V: ContentVersion>: Sized {
    /// A fault, which is not a refusal. See [`Play::Error`](super::Play::Error).
    type Error;

    /// What it takes to reach the table's games. See
    /// [`Play::Host`](super::Play::Host).
    type Host;

    /// One seat's view of the run, lent. A table that holds the run reads it
    /// off the run. One that asks a game lends the last report it has.
    type Seen<'a>: Observed<V>
    where
        Self: 'a;

    /// Starts the run `start` names, with a seat for each of its players,
    /// and returns once every seat has joined.
    fn begin(host: Self::Host, start: &Start<V>) -> Result<Self, Self::Error>;

    /// How many seats the run has.
    fn seats(&self) -> usize;

    /// Where seat `seat` stands, lent and not built.
    fn seen(&mut self, seat: SeatIndex) -> Result<Self::Seen<'_>, Self::Error>;

    /// The moves seat `seat` can make now, in the order a player picks from,
    /// written over `moves`. None while the seat waits on the others: after
    /// it ends the turn or votes, until they do too. A question put to the
    /// seat is answered whatever else it did.
    fn moves(&mut self, seat: SeatIndex, moves: &mut Vec<Decision<V>>) -> Result<(), Self::Error>;

    /// Plays one of seat `seat`'s moves.
    ///
    /// Hands back no report, because building one costs more than most moves
    /// do. A caller that wants one asks [`RunTable::snapshot`].
    fn play(&mut self, seat: SeatIndex, decision: &Decision<V>) -> Result<Settled<V>, Self::Error>;

    /// Where seat `seat` stands, built in full: the report and the question.
    fn snapshot(&mut self, seat: SeatIndex) -> Result<Snapshot<V>, Self::Error>;

    /// Puts the run into a position without playing to it, on seat `seat`'s
    /// machine. A setup that names no player is for that seat's own.
    fn construct(&mut self, seat: SeatIndex, setup: &Setup<V>) -> Result<Settled<V>, Self::Error>;

    /// What the games logged as errors since the last ask, by seat. A table
    /// with no game logs nothing.
    fn logged(&mut self) -> Vec<(SeatIndex, String)> {
        Vec::new()
    }

    /// What the table does as the run ends, told how the run ended. A layer
    /// can say more: a hunt can find the run parted at its end. A layer that
    /// keeps something writes it here.
    fn finish(&mut self, verdict: Verdict) -> Verdict {
        verdict
    }
}

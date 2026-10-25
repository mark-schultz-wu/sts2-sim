//! A co-op run, seen from one machine.
//!
//! In a co-op run every player is a machine of its own. Each machine has a
//! view of the run that the others do not share. A purchase runs its hooks on
//! the buyer's machine alone, so two peers can differ and both be right. So
//! the unit here is a *seat*: one player, and that player's machine.
//!
//! [`Seat`] is what every implementation of a seat answers to, whether it is
//! the real game or a simulator. [`Coop`](super::coop::Coop) seats several of
//! them in one lobby and decides who acts next.
//!
//! A seat's [`Seat::play`] returns once its move is submitted, not once it is
//! resolved. A co-op move can wait on the other seats. Ending the turn waits
//! for every seat to end it, and travel waits for every vote.

use crate::{ContentVersion, Coord, Decision, Refusal, Snapshot, Start};

/// A place in a run's lobby: an index into [`Start::players`]. Seat 0 hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SeatIndex(pub u8);

impl SeatIndex {
    /// The index into [`Start::players`].
    pub fn at(self) -> usize {
        usize::from(self.0)
    }
}

/// What a seat's screens show about one player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Peer {
    /// The player ended the turn and waits for the others.
    pub ended_turn: bool,
    /// Where the player voted to travel, until the run moves.
    pub vote: Option<Coord>,
}

/// What a seat knows about every player in the lobby, itself included.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Peers {
    /// One entry for each seat, in seat order.
    pub seats: Vec<Peer>,
    /// The seats whose choice the game waits for.
    pub awaiting: Vec<SeatIndex>,
    /// A move of this seat's is not back from the host yet.
    pub in_flight: bool,
}

/// Where one seat stands: its own view of the run, and of the others.
#[derive(Debug, Clone, PartialEq)]
pub struct Seated<V: ContentVersion> {
    /// This seat's own player and run, as one machine sees them.
    pub snapshot: Snapshot<V>,
    pub peers: Peers,
}

impl<V: ContentVersion> Seated<V> {
    /// The moves seat `me` makes now, which is none while it waits on the
    /// others.
    ///
    /// A seat waits after it ends the turn or votes, until the others do the
    /// same. It also waits while a move it sent is not back. A question put to
    /// this seat is answered whatever else the seat has done.
    pub fn moves(&self, me: SeatIndex) -> &[Decision<V>] {
        let moves = &self.snapshot.observation.moves;
        if self.peers.in_flight {
            return &[];
        }
        if self.snapshot.asking.is_some() {
            return moves;
        }
        match self.peers.seats.get(me.at()) {
            Some(peer) if peer.ended_turn || peer.vote.is_some() => &[],
            _ => moves,
        }
    }
}

/// What a seat's move came to: where the seat stands after it, or the
/// game's refusal.
pub type SeatAnswered<V> = Result<Seated<V>, Refusal<V>>;

/// One player's machine in a co-op run.
///
/// The co-op counterpart of [`Play`](super::Play): the same three methods,
/// each scoped to one seat.
pub trait Seat<V: ContentVersion>: Sized {
    /// A fault, which is not a refusal. See [`Play::Error`](super::Play::Error).
    type Error;

    /// What it takes to reach this seat's game. See
    /// [`Play::Host`](super::Play::Host).
    type Host;

    /// Takes seat `me` in the run `start` names, and returns once the run has
    /// started.
    ///
    /// A run starts only once every seat has joined, so the seats of one run
    /// begin at the same time. [`Coop::begin`](super::coop::Coop::begin) does
    /// that.
    fn begin(host: Self::Host, start: &Start<V>, me: SeatIndex) -> Result<Self, Self::Error>;

    /// Submits one of this seat's moves.
    ///
    /// Returns once the move is submitted. What the move does can wait on the
    /// other seats.
    fn play(&mut self, decision: &Decision<V>) -> Result<SeatAnswered<V>, Self::Error>;

    /// Where this seat stands, without moving it.
    fn observe(&mut self) -> Result<Seated<V>, Self::Error>;
}

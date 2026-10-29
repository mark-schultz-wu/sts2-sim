//! A co-op run, seen from one machine.
//!
//! In a co-op run every player is a machine of its own. Each machine has a
//! view of the run that the others do not share. A purchase runs its hooks on
//! the buyer's machine alone, so two peers can differ and both be right. So
//! the unit here is a *seat*: one player, and that player's machine.
//!
//! A [`RunTable`](crate::RunTable) holds every seat of a run. [`Seated`] is
//! where one seat stands as its game reports it, its peers included. A co-op
//! move can wait on the other seats. Ending the turn waits for every seat to
//! end it, and travel waits for every vote.

use crate::{ContentVersion, Coord, Decision, Refusal, Snapshot};

/// A place in a run's lobby: an index into [`Start::players`](crate::Start::players). Seat 0 hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SeatIndex(pub u8);

impl SeatIndex {
    /// The index into [`Start::players`](crate::Start::players).
    pub fn at(self) -> usize {
        usize::from(self.0)
    }

    /// Every seat of a lobby of `count`, in order.
    pub fn all(count: usize) -> impl Iterator<Item = SeatIndex> {
        (0..count).map(|at| SeatIndex(u8::try_from(at).expect("at most 256 seats")))
    }

    /// This seat's player's creature. A fight adds every player before
    /// anything else, in seat order, so seat `k`'s creature is `k`.
    pub fn creature(self) -> crate::CombatId {
        crate::CombatId::new(nonmax::NonMaxU8::new(self.0).expect("a seat is below 255"))
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
    /// How many shared actions this machine has taken in from the host, in
    /// order: actions, hook actions and resumes after a choice. Every seat
    /// comes to the same count once it has caught up with the others.
    pub synced: u64,
    /// How many checksums this machine's game generated. Every seat
    /// generates them at the same points, a turn's changes among them. A
    /// seat still running a turn's change has fewer than one past it.
    pub checksums: u64,
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

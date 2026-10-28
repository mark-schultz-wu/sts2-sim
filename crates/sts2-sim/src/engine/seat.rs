//! One seat of a co-op run, on the simulator.
//!
//! A co-op run is a machine for each player, and each machine holds the run
//! as it sees it. Most of what happens runs on every machine alike. Some runs
//! on one alone, such as what a purchase sets off on the buyer's. So a seat
//! here is one machine's run, which takes every player's decisions as that
//! machine receives them.

use sts2_core::{Decision, Rules, SeatIndex, Setup, Snapshot, Start};

use crate::combat::Error;
use crate::run::{Settled, State};

/// The run as seat `me`'s machine holds it.
#[derive(Debug, Clone)]
pub struct SimSeat<V: Rules> {
    state: State<V>,
    me: SeatIndex,
}

impl<V: Rules> SimSeat<V> {
    /// The run `start` names, as `me`'s machine opens it.
    ///
    /// # Panics
    ///
    /// What [`Sim`](super::Sim) panics on as a run begins, and a run of
    /// several players, which nothing here starts yet.
    pub fn begin(start: &Start<V>, me: SeatIndex) -> Self {
        assert!(me.at() < start.players.len(), "seat {} in {start:?}", me.0);
        Self {
            state: super::sim::started(start),
            me,
        }
    }

    /// The seat this machine is.
    pub fn me(&self) -> SeatIndex {
        self.me
    }

    /// Where the run stands, as this seat sees it.
    pub fn observe(&self) -> Snapshot<V> {
        self.state.report(self.me)
    }

    /// [`SimSeat::observe`]'s facts, lent to a comparison without building
    /// the report.
    pub fn observed(&self) -> impl sts2_core::Observed<V> + Copy + '_ {
        self.state.seen_by(self.me)
    }

    /// `decision`, which seat `seat`'s player took, as this machine plays
    /// it. Every seat's decisions reach every machine, its own among them.
    pub fn play(&mut self, seat: SeatIndex, decision: &Decision<V>) -> Result<Settled<V>, Error> {
        super::sim::decide(&mut self.state, seat, decision)
    }

    /// Puts the run into a position, as this machine's game does. A setup
    /// that names no player is for this machine's own.
    pub fn construct(&mut self, step: &Setup<V>) -> Result<Settled<V>, Error> {
        super::sim::constructed(&mut self.state, self.me, step)
    }
}

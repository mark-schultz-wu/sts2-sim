//! A run at a table, on the simulator: one machine's view for each seat.
//!
//! A co-op run is a machine for each player, and each machine holds the run
//! as it sees it. Every player's move reaches every machine. So the table
//! keeps a run for each seat, and plays each move on all of them. A solo run
//! is a table of one, and pays for nothing more.

use smallvec::SmallVec;
use sts2_core::{Decision, Rules, RunTable, SeatIndex, Settled, Setup, Snapshot, Start};

use crate::combat::Error;
use crate::run::{AtSeat, State};

/// The run, as each seat's machine holds it.
#[derive(Debug, Clone)]
pub struct SimTable<V: Rules> {
    /// Seat `k`'s machine is `machines[k]`.
    machines: SmallVec<[State<V>; 1]>,
}

impl<V: Rules> RunTable<V> for SimTable<V> {
    type Error = Error;
    type Host = ();
    type Seen<'a> = AtSeat<'a, V>;

    /// Every machine opens the same run, so the first is drawn and the rest
    /// copy it.
    fn begin(_: Self::Host, start: &Start<V>) -> Result<Self, Self::Error> {
        let first = super::sim::started(start);
        let mut machines: SmallVec<[State<V>; 1]> = SmallVec::with_capacity(start.players.len());
        for _ in 1..start.players.len() {
            machines.push(first.clone());
        }
        machines.push(first);
        for (at, machine) in machines.iter_mut().enumerate() {
            machine.play_as(SeatIndex(
                u8::try_from(at).expect("a lobby's seats fit a u8"),
            ));
        }
        Ok(Self { machines })
    }

    fn seats(&self) -> usize {
        self.machines.len()
    }

    fn seen(&mut self, seat: SeatIndex) -> Result<Self::Seen<'_>, Self::Error> {
        Ok(self.machines[seat.at()].seen_by(seat))
    }

    /// The moves seat `seat`'s own machine lists. That list keeps a move to
    /// travel after the seat voted, as the game's report does. The seat
    /// waits on the rest all the same, until its vote is counted.
    fn moves(&mut self, seat: SeatIndex, moves: &mut Vec<Decision<V>>) -> Result<(), Self::Error> {
        moves.clear();
        let machine = &self.machines[seat.at()];
        if machine.pending(seat).is_none() && machine.voted(seat) {
            return Ok(());
        }
        let mut legal = crate::Legal::new();
        machine.legal_into(seat, &mut legal);
        moves.extend(legal);
        Ok(())
    }

    /// Seat `seat`'s move on every machine, its own first. What its own
    /// machine says is the answer.
    ///
    /// # Panics
    ///
    /// Another machine refusing the move, or faulting on it, where the seat's
    /// own took it. The machines of one run never part on a move.
    fn play(&mut self, seat: SeatIndex, decision: &Decision<V>) -> Result<Settled<V>, Self::Error> {
        // A travel can enter a room, which syncs every machine from how each
        // stood before it.
        if self.machines.len() > 1 && matches!(decision, Decision::Travel { .. }) {
            let before: std::sync::Arc<[State<V>]> = self.machines.iter().cloned().collect();
            for machine in &mut self.machines {
                machine.sync_from(Some(std::sync::Arc::clone(&before)));
            }
        }
        let own = self.decided(seat, decision);
        for machine in &mut self.machines {
            machine.sync_from(None);
        }
        own
    }

    fn snapshot(&mut self, seat: SeatIndex) -> Result<Snapshot<V>, Self::Error> {
        Ok(self.machines[seat.at()].report(seat))
    }

    fn construct(&mut self, seat: SeatIndex, setup: &Setup<V>) -> Result<Settled<V>, Self::Error> {
        super::sim::constructed(&mut self.machines[seat.at()], seat, setup)
    }
}

impl<V: Rules> SimTable<V> {
    /// Seat `seat`'s move on every machine, its own first.
    fn decided(&mut self, seat: SeatIndex, decision: &Decision<V>) -> Result<Settled<V>, Error> {
        // `NetDiscardPotionGameAction` names the slot as the player's own
        // machine holds it.
        let slot = match decision {
            Decision::DiscardPotion { index } => self.machines[seat.at()].belt_slot(seat, *index),
            _ => None,
        };
        let own = super::sim::decide(&mut self.machines[seat.at()], seat, decision)?;
        if own.is_err() {
            return Ok(own);
        }
        for (at, machine) in self.machines.iter_mut().enumerate() {
            // A machine whose player has won shows the end of the run, and
            // what the other players still choose changes nothing it shows.
            if at == seat.at() || machine.run_won() {
                continue;
            }
            if let Some(slot) = slot {
                machine.discard_slot(seat, slot);
                continue;
            }
            match super::sim::decide(machine, seat, decision) {
                Ok(Ok(_)) => {}
                other => unimplemented!(
                    "seat {at}'s machine answering {other:?} to seat {}'s {decision:?}, \
                     which its own took",
                    seat.0
                ),
            }
        }
        // What the seat's machine sent once a purchase there was over, which
        // reaches every other machine.
        for purchase in self.machines[seat.at()].take_sent() {
            for (at, machine) in self.machines.iter_mut().enumerate() {
                if at != seat.at() && !machine.run_won() {
                    machine.receive(purchase.clone());
                }
            }
        }
        Ok(own)
    }
}

//! Moving to a map point, and entering what waits there.

use sts2_core::{Applied, Coord, MapPointKinds, Reason, Rules};

use super::State;
use crate::combat;

/// What waits at a reachable map point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Destination<V: Rules> {
    Monster(V::Encounter),
    Other(V::MapPointType),
}

/// A travel the map allows, resolved but not yet taken.
///
/// Split from taking it so that entering the room can fail after the move is
/// known to be legal, without a half-moved run being observable.
pub(crate) struct Move<V: Rules> {
    to: Coord,
    destination: Destination<V>,
}

impl<V: Rules> State<V> {
    /// Validate a move and resolve what it will enter, without changing state.
    ///
    /// Keeping this read-only makes travel transactional. Entering a room can
    /// still fail while its rules are incomplete, and a failed decision must
    /// leave the run where it was.
    pub(crate) fn resolve(&self, to: Coord) -> Result<Move<V>, Reason<V>> {
        let reported = self.report();
        let reachable = reported
            .observation
            .map
            .travelable
            .iter()
            .find(|reachable| reachable.at == to)
            .ok_or(Reason::NotTravelable { at: to })?;
        let destination = if reachable.kind == V::MapPointType::MONSTER {
            let act: usize = num_traits::cast(reported.observation.act)
                .expect("an act number fits an addressable list");
            let encounter = self
                .normal_encounter(act)
                .expect("a reachable monster room has a rolled encounter");
            Destination::Monster(encounter)
        } else {
            Destination::Other(reachable.kind)
        };
        Ok(Move { to, destination })
    }

    /// Take a resolved move: build the room's state, then commit the move.
    ///
    /// The room comes first, and nothing before the last block touches the
    /// run. A room the simulator cannot build yet reports which one, and the
    /// caller still holds the run it had.
    pub(crate) fn travel(&mut self, moving: Move<V>) -> Result<Applied<V>, crate::engine::Error> {
        let Move { to, destination } = moving;
        let Destination::Monster(encounter) = destination else {
            return Err(crate::engine::Error::NotImplemented("this room"));
        };

        let (combat, counters) = combat::enter::<V>(
            self.raw_seed(),
            self.character(),
            self.ascension(),
            encounter,
            &self.report().observation,
        )?;

        self.enter_combat(to, counters, combat);
        Ok(self.report())
    }
}

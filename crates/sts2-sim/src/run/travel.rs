//! Resolving a map destination before entering its room.

use sts2_core::{Coord, MapPointKinds, Reason, Rules};

use super::State;

/// What waits at a reachable map point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Destination<V: Rules> {
    Monster(V::Encounter),
    Other(V::MapPointType),
}

impl<V: Rules> State<V> {
    /// Validate a move and resolve what it will enter, without changing state.
    ///
    /// Keeping this read-only makes travel transactional. Entering a room can
    /// still fail while its rules are incomplete, and a failed decision must
    /// leave the run where it was.
    pub(crate) fn destination(&self, to: Coord) -> Result<Destination<V>, Reason<V>> {
        let reported = self.report();
        let reachable = reported.observation.map.travelable
            .iter()
            .find(|reachable| reachable.at == to)
            .ok_or(Reason::NotTravelable { at: to })?;

        if reachable.kind == V::MapPointType::MONSTER {
            let act = reported.observation.act as usize;
            let encounter = self
                .normal_encounter(act)
                .expect("a reachable monster room has a rolled encounter");
            Ok(Destination::Monster(encounter))
        } else {
            Ok(Destination::Other(reachable.kind))
        }
    }
}

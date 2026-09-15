//! Moving to a map point, and entering what waits there.

use sts2_core::rng::GameRng;
use sts2_core::{Applied, Coord, MapPointKinds, Reason, Rules, RunStreamKinds};

use super::State;
use super::state::EncounterQueue;

/// `TreasureRoom`'s normal relic-rarity weights.
const COMMON_RELIC_CHANCE: f64 = 0.5;
const UNCOMMON_RELIC_CHANCE: f64 = 0.33;

/// What waits at a reachable map point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Destination<V: Rules> {
    Combat {
        encounter: V::Encounter,
        queue: EncounterQueue,
    },
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
        let destination = if reachable.kind == V::MapPointType::MONSTER
            || reachable.kind == V::MapPointType::ELITE
        {
            let act: usize = num_traits::cast(reported.observation.act)
                .expect("an act number fits an addressable list");
            let (encounter, queue) = if reachable.kind == V::MapPointType::MONSTER {
                (
                    self.normal_encounter(act)
                        .expect("a reachable monster room has a rolled encounter"),
                    EncounterQueue::Normal,
                )
            } else {
                (
                    self.elite_encounter(act)
                        .expect("a reachable elite room has a rolled encounter"),
                    EncounterQueue::Elite,
                )
            };
            Destination::Combat { encounter, queue }
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
    pub(crate) fn travel(&mut self, moving: Move<V>) -> Result<Applied<V>, crate::combat::Error> {
        let Move { to, destination } = moving;
        if let Destination::Other(kind) = destination {
            if kind == V::MapPointType::TREASURE {
                let mut streams = self.streams().clone();
                let roll = streams
                    .run(V::RunRngType::TREASURE_ROOM_RELICS)
                    .next_double();
                let rarity = if roll < COMMON_RELIC_CHANCE {
                    <super::Rarity<V> as sts2_core::RarityKinds>::COMMON
                } else if roll < COMMON_RELIC_CHANCE + UNCOMMON_RELIC_CHANCE {
                    <super::Rarity<V> as sts2_core::RarityKinds>::UNCOMMON
                } else {
                    <super::Rarity<V> as sts2_core::RarityKinds>::RARE
                };
                let relic = self.next_shared_relic(rarity);
                self.enter_treasure(to, relic, streams);
                return Ok(self.report());
            }
            if kind != V::MapPointType::UNKNOWN {
                unimplemented!("entering {destination:?}");
            }
            let (mut events_used, visited, mut odds) = self.event_state();
            let mut streams = self.streams().clone();
            let roll = streams.run(V::RunRngType::UNKNOWN_MAP_POINT).next_double() as f32;
            let mut cumulative = 0.0;
            for probability in odds {
                cumulative += probability;
                if roll <= cumulative {
                    unimplemented!("a non-event unknown room");
                }
            }
            for (probability, increase) in odds.iter_mut().zip([0.1, 0.02, 0.03]) {
                *probability += increase;
            }
            let events = self.event_candidates();
            let event = (0..events.len())
                .find_map(|_| {
                    let candidate = events[events_used % events.len()];
                    events_used += 1;
                    (!visited.contains(&candidate)
                        && V::event_allowed(candidate, &self.report().observation))
                    .then_some(candidate)
                })
                .unwrap_or_else(|| unimplemented!("an exhausted event queue"));
            let mut event_rng =
                crate::event::stream::<V, crate::rng::MegaRng>(self.raw_seed(), 0, event);
            let state = V::enter_event(event, self, &mut event_rng);
            self.enter_event(
                to,
                super::state::EnteredEvent {
                    event,
                    state,
                    rng: event_rng,
                    streams,
                    events_used,
                    unknown_odds: odds,
                },
            );
            return Ok(self.report());
        }
        let Destination::Combat { encounter, queue } = destination else {
            unreachable!()
        };

        let (combat, counters) = self.open_combat(encounter, &self.report().observation)?;

        self.enter_combat(to, queue, counters, combat);
        Ok(self.report())
    }
}

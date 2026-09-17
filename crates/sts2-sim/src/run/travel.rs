//! Moving to a map point, and entering what waits there.

use sts2_core::rng::GameRng;
use sts2_core::{AscensionKinds, Coord, Decided, MapPointKinds, Reason, Rules, RunStreamKinds};

use super::State;
use super::state::{EncounterQueue, Location};

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
        let travelable = self.travelable();
        let reachable = travelable
            .iter()
            .find(|reachable| reachable.at == to)
            .ok_or(Reason::NotTravelable { at: to })?;
        let fights = [
            V::MapPointType::MONSTER,
            V::MapPointType::ELITE,
            V::MapPointType::BOSS,
        ];
        let destination = if fights.contains(&reachable.kind) {
            let act = self.act();
            let (encounter, queue) = if reachable.kind == V::MapPointType::MONSTER {
                (
                    self.normal_encounter(act)
                        .expect("a reachable monster room has a rolled encounter"),
                    EncounterQueue::Normal,
                )
            } else if reachable.kind == V::MapPointType::ELITE {
                (
                    self.elite_encounter(act)
                        .expect("a reachable elite room has a rolled encounter"),
                    EncounterQueue::Elite,
                )
            } else {
                (
                    self.boss_encounter(act)
                        .expect("an act rolled the boss that ends it"),
                    EncounterQueue::Boss,
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
    pub(crate) fn travel(&mut self, moving: Move<V>) -> Result<Decided, crate::combat::Error> {
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
                return Ok(Decided::Resolved);
            }
            if kind == V::MapPointType::REST_SITE {
                self.enter_rest_site(to);
                return Ok(Decided::Resolved);
            }
            if kind == V::MapPointType::SHOP {
                let shop = self.stocked_shop();
                self.enter(Location::MapPoint(to), shop);
                return Ok(Decided::Resolved);
            }
            if kind == V::MapPointType::ANCIENT {
                // `AncientEventModel.BeforeEventStarted`, which heals what the
                // run is missing before the ancient says anything. The rung
                // that heals four fifths of it instead is not ported.
                if self
                    .ascension()
                    .has(<V::AscensionLevel as AscensionKinds>::WEARY_TRAVELER)
                {
                    unimplemented!("an ancient healing a weary traveller");
                }
                self.heal_fully();
                // The act rolled which ancient it opens on when it rolled its
                // rooms, so nothing is drawn for it here. The question mark
                // odds and the event queue are the unknown rooms' own, and an
                // ancient is not one of them.
                let event = self.act_ancient();
                let mut event_rng =
                    crate::event::stream::<V, crate::rng::MegaRng>(self.raw_seed(), 0, event);
                let state = V::enter_event(event, self, &mut event_rng);
                let (events_used, _, unknown_odds) = self.event_state();
                let streams = self.streams().clone();
                self.enter_event(
                    to,
                    super::state::EnteredEvent {
                        event,
                        state,
                        rng: event_rng,
                        streams,
                        events_used,
                        unknown_odds,
                    },
                );
                return Ok(Decided::Resolved);
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
                    (!visited.contains(&candidate) && V::event_allowed(candidate, self))
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
            return Ok(Decided::Resolved);
        }
        let Destination::Combat { encounter, queue } = destination else {
            unreachable!()
        };

        let fight = self.create_fight(encounter, queue)?;
        self.enter_fight(fight, Location::MapPoint(to));
        Ok(Decided::Resolved)
    }
}

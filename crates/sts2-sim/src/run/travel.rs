//! Moving to a map point, and entering what waits there.

use sts2_core::{Coord, Decided, MapPointKinds, Reason, Rules};

use super::State;
use super::state::{EncounterQueue, Location, UnknownRoom};

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
    /// A treasure room at `to`, entered on the streams the move resolved.
    fn treasure(&mut self, to: Coord, streams: crate::stream::Streams<V>) {
        self.enter_treasure(Location::MapPoint(to), streams);
    }

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
    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn travel(&mut self, moving: Move<V>) -> Result<Decided, crate::combat::Error> {
        let Move { to, destination } = moving;
        if let Destination::Other(kind) = destination {
            if kind == V::MapPointType::TREASURE {
                self.treasure(to, *self.streams());
                return Ok(Decided::Resolved);
            }
            if kind == V::MapPointType::REST_SITE {
                self.enter_rest_site(Location::MapPoint(to));
                return Ok(Decided::Resolved);
            }
            if kind == V::MapPointType::SHOP {
                let shop = self.stocked_shop();
                self.enter(Location::MapPoint(to), shop);
                return Ok(Decided::Resolved);
            }
            if kind == V::MapPointType::ANCIENT {
                // `AncientEventModel.BeforeEventStarted`, which heals before
                // the ancient says anything.
                self.ancient_heal();
                // The act rolled which ancient it opens on when it rolled its
                // rooms, so nothing is drawn for it here. The question mark
                // odds and the event queue are the unknown rooms' own, and an
                // ancient is not one of them.
                let event = self.act_ancient();
                let mut event_rng =
                    crate::event::stream::<V, crate::rng::MegaRng>(self.raw_seed(), 0, event);
                let mut streams = *self.streams();
                let floor = sts2_core::EventView::floor(&*self) + num_traits::one();
                let (state, layout) =
                    self.open_event(event, floor, &mut event_rng, &mut streams)?;
                let (events_used, _, unknown_odds) = self.event_state();
                self.enter_event(
                    to,
                    super::state::EnteredEvent {
                        event,
                        state,
                        layout,
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
            let mut streams = *self.streams();
            let rolled = self.roll_unknown_room(to, &mut streams);
            let (mut events_used, visited, odds) = self.event_state();
            if let Some(room) = rolled {
                if room == UnknownRoom::Treasure {
                    self.treasure(to, streams);
                    return Ok(Decided::Resolved);
                }
                self.drew(streams);
                if room == UnknownRoom::Shop {
                    let shop = self.stocked_shop();
                    self.enter(Location::MapPoint(to), shop);
                    return Ok(Decided::Resolved);
                }
                let (encounter, queue) = match room {
                    // An elite, where a modifier lets the odds reach one.
                    UnknownRoom::Elite => (
                        self.elite_encounter(self.act())
                            .expect("an act rolled the fights its elite rooms deal"),
                        EncounterQueue::Elite,
                    ),
                    _ => (
                        self.normal_encounter(self.act())
                            .expect("an act rolled the fights its monster rooms deal"),
                        EncounterQueue::Normal,
                    ),
                };
                let at = Location::MapPoint(to);
                let fight = self.create_fight(encounter, queue, at)?;
                return self.enter_fight(fight, at);
            }
            // `RoomSet.EnsureNextEventIsValid`, which passes over an event
            // that is not allowed or was seen already. The one it stops at
            // is counted as its room is entered.
            let events = self.event_candidates();
            let event = (0..events.len())
                .find_map(|_| {
                    let candidate = events[events_used % events.len()];
                    if V::event_allowed(candidate, self) && !visited.contains(&candidate) {
                        return Some(candidate);
                    }
                    events_used += 1;
                    None
                })
                .unwrap_or_else(|| unimplemented!("an exhausted event queue"));
            // `Hook.ModifyNextEvent`, which a deck card can answer.
            let event = self
                .deck_hearing(sts2_core::ModelHook::ModifyNextEvent)
                .fold(event, |event, card| {
                    V::deck_card_next_event(card, self.act(), event)
                });
            let mut event_rng =
                crate::event::stream::<V, crate::rng::MegaRng>(self.raw_seed(), 0, event);
            let floor = sts2_core::EventView::floor(&*self) + num_traits::one();
            let (state, layout) = self.open_event(event, floor, &mut event_rng, &mut streams)?;
            self.enter_event(
                to,
                super::state::EnteredEvent {
                    event,
                    state,
                    layout,
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

        let at = Location::MapPoint(to);
        let fight = self.create_fight(encounter, queue, at)?;
        self.enter_fight(fight, at)
    }
}

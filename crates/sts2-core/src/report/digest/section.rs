//! One observation, as a hash per part of it.
//!
//! [`Observation::rules_digest`] answers whether two runs agree. It cannot say
//! where they parted, because eight bytes over the whole state say nothing
//! about which field moved. A scenario that fails wants to name the part.
//!
//! So the same walk runs once per section, into a sink of its own. Each is the
//! same eight bytes over less. A comparison that finds one section differing
//! and the rest agreeing has localised the divergence without keeping a copy
//! of what the other side said.
//!
//! The numbers are the format. A section's number is written into a scenario's
//! expectations, so it is fixed here and never derived from position. Adding a
//! section takes the next number.

use super::{Digest, Sink};
use std::borrow::Borrow;

use crate::{ContentVersion, Observed, ObservedFight};

/// One part of an observation, as the comparison names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Section {
    /// The run's own scalars: floor, health, gold, whether a fight is on.
    Run = 0,
    Relics = 1,
    Potions = 2,
    Deck = 3,
    Map = 4,
    /// How far each random stream has been drawn.
    Rng = 5,
    Room = 6,
    /// The fight's own scalars: turn, round, phase, pools, encounter, orbs.
    Turn = 7,
    Creatures = 8,
    Hand = 9,
    /// Draw, discard and exhaust.
    Piles = 10,
    /// What each card in hand can do, and why the rest cannot.
    Legal = 11,
}

impl Section {
    /// Every section, in the order a report lists them.
    pub const ALL: [Section; 12] = [
        Section::Run,
        Section::Relics,
        Section::Potions,
        Section::Deck,
        Section::Map,
        Section::Rng,
        Section::Room,
        Section::Turn,
        Section::Creatures,
        Section::Hand,
        Section::Piles,
        Section::Legal,
    ];

    /// The number this section is written down as.
    pub const fn number(self) -> u8 {
        self as u8
    }

    /// The name a failure prints.
    pub const fn name(self) -> &'static str {
        match self {
            Section::Run => "run",
            Section::Relics => "relics",
            Section::Potions => "potions",
            Section::Deck => "deck",
            Section::Map => "map",
            Section::Rng => "rng",
            Section::Room => "room",
            Section::Turn => "turn",
            Section::Creatures => "creatures",
            Section::Hand => "hand",
            Section::Piles => "piles",
            Section::Legal => "legal",
        }
    }
}

impl std::fmt::Display for Section {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// This observation, as one digest per section.
///
/// Prose is left out, for the reason [`super::Digested::rules_digest`] leaves
/// it out. A simulator has no localisation tables, and holding it to a sentence
/// tests a text formatter.
pub(super) fn sections<V: ContentVersion>(
    observed: &impl Observed<V>,
) -> [(Section, Digest); Section::ALL.len()] {
    let fight = observed.combat();
    Section::ALL.map(|section| {
        let mut sink = Sink::rules();
        match section {
            Section::Run => {
                sink.number(&observed.act());
                sink.number(&observed.floor());
                sink.number(&observed.hp());
                sink.number(&observed.max_hp());
                sink.bool(observed.alive());
                sink.bool(observed.run_over());
                sink.maybe(observed.victory(), |sink, won| sink.bool(won));
                sink.number(&observed.gold());
                sink.number(&observed.potion_slots());
                sink.bool(observed.in_combat());
            }
            Section::Relics => {
                sink.seq(observed.relics(), |sink, relic| {
                    sink.carried(relic.borrow())
                });
            }
            Section::Potions => {
                sink.seq(observed.potions(), |sink, potion| {
                    sink.held(potion.borrow())
                });
            }
            Section::Deck => sink.seq(observed.deck(), |sink, card| sink.card(&card)),
            Section::Map => sink.map(observed),
            Section::Rng => sink.counters_of(observed),
            Section::Room => sink.maybe(observed.room(), |sink, room| sink.room(room.borrow())),
            Section::Turn => sink.maybe(fight.as_ref(), |sink, fight| {
                sink.fight_scalars(fight);
                sink.orbs(fight.orbs());
            }),
            Section::Creatures => sink.maybe(fight.as_ref(), |sink, fight| {
                sink.seq(fight.creatures(), |sink, creature| sink.creature(&creature));
            }),
            Section::Hand => sink.maybe(fight.as_ref(), |sink, fight| {
                sink.seq(fight.hand(), |sink, card| sink.card(&card));
            }),
            Section::Piles => sink.maybe(fight.as_ref(), |sink, fight| {
                sink.seq(fight.draw(), |sink, card| sink.card(&card));
                sink.seq(fight.discard(), |sink, card| sink.card(&card));
                sink.seq(fight.exhaust(), |sink, card| sink.card(&card));
            }),
            Section::Legal => sink.maybe(fight.as_ref(), |sink, fight| {
                sink.seq(fight.legal(), |sink, playable| sink.playable(&playable));
            }),
        }
        (section, Digest(sink.finish()))
    })
}

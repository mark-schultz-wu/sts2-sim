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
use crate::{ContentVersion, Observation};

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

impl<V: ContentVersion> Observation<V> {
    /// This observation, as one digest per section.
    ///
    /// Prose is left out, for the reason [`Observation::rules_digest`] leaves
    /// it out. A simulator has no localisation tables, and holding it to a
    /// sentence tests a text formatter.
    ///
    /// Every field reaches exactly one section. The destructuring below is
    /// exhaustive, so a field added to an observation fails to compile until
    /// somebody says which section it belongs to.
    pub fn sections(&self) -> [(Section, Digest); Section::ALL.len()] {
        let Observation {
            act,
            floor,
            hp,
            max_hp,
            alive,
            run_over,
            victory,
            gold,
            relics,
            potions,
            potion_slots,
            deck,
            in_combat,
            map,
            rng,
            room,
            combat,
        } = self;

        let fight = combat.as_ref();
        Section::ALL.map(|section| {
            let mut sink = Sink::rules();
            match section {
                Section::Run => {
                    sink.number(act);
                    sink.number(floor);
                    sink.number(hp);
                    sink.number(max_hp);
                    sink.bool(*alive);
                    sink.bool(*run_over);
                    sink.option(victory, |sink, won| sink.bool(*won));
                    sink.number(gold);
                    sink.number(potion_slots);
                    sink.bool(*in_combat);
                }
                Section::Relics => sink.list(relics, |sink, relic| sink.carried(relic)),
                Section::Potions => sink.list(potions, |sink, potion| sink.held(potion)),
                Section::Deck => sink.list(deck, |sink, card| sink.card(card)),
                Section::Map => sink.map(map),
                Section::Rng => sink.counters_of(rng),
                Section::Room => sink.option(room, |sink, room| sink.room(room)),
                Section::Turn => sink.option(&fight, |sink, fight| {
                    sink.fight_scalars(fight);
                    sink.orbs(&fight.orbs);
                }),
                Section::Creatures => sink.option(&fight, |sink, fight| {
                    sink.list(&fight.creatures, |sink, creature| sink.creature(creature));
                }),
                Section::Hand => sink.option(&fight, |sink, fight| {
                    sink.list(&fight.hand, |sink, card| sink.card(card));
                }),
                Section::Piles => sink.option(&fight, |sink, fight| {
                    for pile in [&fight.draw, &fight.discard, &fight.exhaust] {
                        sink.list(pile, |sink, card| sink.card(card));
                    }
                }),
                Section::Legal => sink.option(&fight, |sink, fight| {
                    sink.list(&fight.legal, |sink, playable| sink.playable(playable));
                }),
            }
            (section, Digest(sink.finish()))
        })
    }
}

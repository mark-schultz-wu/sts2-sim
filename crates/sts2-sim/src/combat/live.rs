//! The fight the simulator owns.
//!
//! [`sts2_core::Combat`] is a snapshot: what a fight looks like from outside.
//! This is the fight itself. Rules mutate it, nothing clones it to hand out,
//! and it carries no report.
//!
//! What it holds beyond the snapshot is the move graphs and the skipped ticks.
//! An observation says what a monster does next, not how its graph got there.
//!
//! What it does not hold is [`sts2_core::Playable`]. Every card in hand with
//! its previews is derived from the fields below, so [`Live::report`] builds
//! it and nothing else does.

use num_traits::CheckedAdd;
use sts2_core::{
    CardId, CardIdent, CardNumber, CardTable, CardVars, Combat, CombatId, Creature, Orbs, Rules,
    SideKinds,
};

use super::monster::Machine;

/// A card play that stopped on a question, and what it needs to go on.
///
/// The card is out of the hand and in the play pile, which is where
/// `AddDuringManualCardPlay` put it. It stays there, held here rather than in
/// a pile, until the question is answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Asking<V: Rules> {
    pub(super) card: CardInstance<V>,
    /// The creature the play was pointed at, which the rest of it still needs.
    pub(super) target: Option<CombatId>,
    /// Where the card stopped, in the card's own words.
    pub(super) at: V::Resume,
    /// The pile the selection reads.
    pub(super) pile: sts2_core::Pile,
    /// What the selection takes.
    pub(super) selection: sts2_core::Selection,
    /// Whether the fight was already won when the play stopped.
    pub(super) won: bool,
}

/// Mints the ids a fight names its cards by.
///
/// One counter, and nothing reuses a number it gave out. A fight that runs out
/// stops the run rather than name two cards the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CardIds<V: Rules>(V::CardSerial);

impl<V: Rules> CardIds<V> {
    pub(super) fn new() -> Self {
        Self(num_traits::zero())
    }

    pub(super) fn mint(&mut self) -> CardId<V> {
        let next = self.0;
        self.0 = next
            .checked_add(&num_traits::one())
            .unwrap_or_else(|| unimplemented!("a fight holding more cards than it can name"));
        CardId::new(next)
    }
}

/// One mutable card in a combat pile.
///
/// A pile holds copies rather than ids, because a fight writes on a card. Two
/// copies of one card are two objects. A rule that raises the damage on one
/// leaves the other where it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CardInstance<V: Rules> {
    /// Which copy this is, for as long as the fight holds it.
    pub(super) id: CardId<V>,
    pub(super) ident: CardIdent<V>,
    /// The numbers this copy prints, which start as the table's and move.
    pub(super) vars: Vec<CardNumber<V::CardVar, V::Number>>,
    /// `CardModel.EnergyCost` and `StarCost` for this copy.
    ///
    /// Per copy, because an upgrade writes on them. The table prices a fresh
    /// one, and `UpgradeBy` moves the copy alone.
    pub(super) cost: sts2_core::Cost,
    pub(super) star_cost: sts2_core::Cost,
}

impl<V: Rules> CardInstance<V> {
    /// A fresh copy, printing what the table prints.
    pub(super) fn new(ident: CardIdent<V>, id: CardId<V>) -> Self {
        let vars = ident
            .card
            .vars()
            .iter()
            .map(|var| CardNumber {
                id: var.id,
                // A build can store exact fractions where this slice only
                // evaluates whole-number effects.
                base: num_traits::NumCast::from(var.base).unwrap_or_else(|| {
                    unimplemented!("the fractional {} this card prints", var.id.name())
                }),
                calculated: var.calculated,
                modifier: var.modifier,
            })
            .collect();
        let mut card = Self {
            id,
            ident,
            vars,
            cost: ident.card.cost(),
            star_cost: ident.card.star_cost(),
        };
        if card.ident.upgrade != 0 {
            assert_eq!(card.ident.upgrade, 1, "only first upgrades are ported");
            V::upgrade_card(card.ident.card, card.face());
        }
        card
    }

    /// This copy's own face, which is what an upgrade writes on.
    fn face(&mut self) -> sts2_core::Face<'_, V> {
        sts2_core::Face {
            numbers: &mut self.vars,
            cost: &mut self.cost,
        }
    }

    pub(super) fn upgrade(&mut self) {
        // `CardModel.IsUpgradable`, which a card already at its own maximum is
        // not. A status card prints no upgrade at all.
        if self.ident.upgrade < self.ident.card.max_upgrade() {
            V::upgrade_card(self.ident.card, self.face());
            self.ident.upgrade += 1;
        }
    }
}

/// One number a power counts on itself, and the creature carrying it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Counter<V: Rules> {
    pub(crate) creature: CombatId,
    pub(crate) power: V::Power,
    pub(crate) amount: V::Number,
}

/// What one power counts on itself, or zero for one counting nothing.
pub(crate) fn counted<V: Rules>(
    counters: &[Counter<V>],
    creature: CombatId,
    power: V::Power,
) -> V::Number {
    counters
        .iter()
        .find(|held| held.creature == creature && held.power == power)
        .map_or_else(num_traits::zero, |held| held.amount)
}

/// A fight in progress.
#[derive(Debug, Clone)]
pub(crate) struct Live<V: Rules> {
    /// What the run is being played at, which a card or a move reads for the
    /// numbers the level moves.
    pub(super) ascension: V::AscensionLevel,
    pub(super) turn: V::Count,
    pub(super) phase: V::PlayerTurnPhase,
    pub(super) energy: V::Count,
    pub(super) stars: V::Count,
    pub(super) round: V::Count,
    /// The encounter this active fight belongs to.
    pub(super) encounter: V::Encounter,
    pub(super) hand: Vec<CardInstance<V>>,
    pub(super) draw: Vec<CardInstance<V>>,
    pub(super) discard: Vec<CardInstance<V>>,
    pub(super) exhaust: Vec<CardInstance<V>>,
    /// Deck cards a combat command removed permanently while this fight ran.
    pub(super) removed_from_deck: Vec<CardIdent<V>>,
    pub(super) creatures: Vec<Creature<V>>,
    pub(super) orbs: Option<Orbs<V>>,
    /// One per monster in the fight, in the order the encounter fields them.
    pub(super) machines: Vec<Machine<V>>,
    /// `PowerModel.SkipNextDurationTick`, as the powers holding it.
    ///
    /// A debuff landing on a player-side creature does not count down at the
    /// end of the turn it landed in. The game keeps that on the power
    /// instance. The observation's [`sts2_core::Stack`] is the game's own wire
    /// shape and carries no such field, so the fight keeps it beside them.
    pub(super) skip_tick: Vec<(CombatId, V::Power)>,
    /// `PowerModel.DynamicVars`, as the powers counting on themselves.
    ///
    /// A power counts here rather than in the stack the creature carries it
    /// as. The reason is `skip_tick`'s: the wire's [`sts2_core::Stack`]
    /// carries the amount and nothing else. Slow counts the cards played
    /// against it in one of these.
    pub(super) counted: Vec<Counter<V>>,
    /// The ids this fight has given out, and the next one it will.
    pub(super) card_ids: CardIds<V>,
    /// A card play waiting on an answer, which is the only thing the fight
    /// will take while it is set.
    pub(super) asking: Option<Asking<V>>,
}

impl<V: Rules> Live<V> {
    pub(crate) fn removed_from_deck(&self) -> &[CardIdent<V>] {
        &self.removed_from_deck
    }

    /// The card-selection question on which this fight is parked.
    pub(crate) fn question(&self) -> Option<sts2_core::Question<V>> {
        let asked = self.asking.as_ref()?;
        let (min, max) = match asked.selection {
            sts2_core::Selection::Exactly(count) => {
                let count = u32::try_from(count.get()).expect("a selection count fits the wire");
                (count, count)
            }
            sts2_core::Selection::UpTo(count) => (
                0,
                u32::try_from(count.get()).expect("a selection count fits the wire"),
            ),
        };
        Some(sts2_core::Question::SelectCards {
            min,
            max,
            // The content id does not carry display text. The selector has no
            // prompt either, so this stays absent as it does for other
            // unattributed selections.
            asker: None,
            options: super::play::pile(self, asked.pile)
                .iter()
                .enumerate()
                .map(|(index, card)| sts2_core::CardOption {
                    index,
                    card: card.ident,
                    text: None,
                })
                .collect(),
        })
    }

    /// The player, which every fight has one of.
    pub(crate) fn player(&self) -> &Creature<V> {
        self.creatures
            .iter()
            .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
            .expect("a fight has the player in it")
    }

    /// This fight as a snapshot, and the only way out of here.
    ///
    /// The one place a `Combat` is made. The hand is held to the rules here
    /// and nowhere else, so a decision nobody observes costs none of that.
    ///
    /// `Attack::damage` comes out as the number that will land, not the one
    /// the move prints. The game works it out on every read.
    pub(crate) fn report(&self) -> Combat<V> {
        let mut creatures = self.creatures.clone();
        let player = self
            .creatures
            .iter()
            .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
            .map(|creature| creature.id);
        for creature in &mut creatures {
            let (Some(player), Some(next)) = (player, &mut creature.next_move) else {
                continue;
            };
            for intent in &mut next.intents {
                if let Some(attack) = &mut intent.attack {
                    attack.damage = super::board::modified::<V>(
                        &self.creatures,
                        &self.counted,
                        creature.id,
                        player,
                        attack.damage,
                    );
                }
            }
        }
        Combat {
            turn: self.turn,
            phase: self.phase,
            energy: self.energy,
            stars: self.stars,
            round: self.round,
            encounter: Some(self.encounter),
            hand: self.hand.iter().map(|card| card.ident).collect(),
            draw: self.draw.iter().map(|card| card.ident).collect(),
            discard: self.discard.iter().map(|card| card.ident).collect(),
            exhaust: self.exhaust.iter().map(|card| card.ident).collect(),
            creatures,
            legal: super::legal::hand::<V>(
                &self.hand,
                &self.creatures,
                &self.counted,
                self.energy,
                self.stars,
            ),
            orbs: self.orbs.clone(),
        }
    }

    /// Consume a won fight into the report that remains on its reward screen.
    pub(crate) fn finish(mut self) -> Combat<V> {
        self.phase = <V::PlayerTurnPhase as sts2_core::TurnPhaseKinds>::NONE;
        self.hand.clear();
        self.draw.clear();
        self.discard.clear();
        self.exhaust.clear();
        let player = self
            .creatures
            .iter_mut()
            .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
            .expect("a fight has the player in it");
        player.powers.clear();
        player.block = num_traits::zero();
        self.report()
    }
}

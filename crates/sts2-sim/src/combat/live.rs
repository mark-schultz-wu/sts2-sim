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

use sts2_core::{
    CardIdent, CardTable, CardVars, Combat, CombatId, Creature, Orbs, Printed, Rules, SideKinds,
};

use super::monster::Machine;

/// One mutable card in a combat pile.
///
/// A pile holds copies rather than ids, because a fight writes on a card. Two
/// copies of one card are two objects. A rule that raises the damage on one
/// leaves the other where it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CardInstance<V: Rules> {
    pub(super) ident: CardIdent<V>,
    /// The numbers this copy prints, which start as the table's and move.
    pub(super) vars: Vec<Printed<V::CardVar, V::Number>>,
}

impl<V: Rules> CardInstance<V> {
    /// A fresh copy, printing what the table prints.
    pub(super) fn new(ident: CardIdent<V>) -> Self {
        let vars = ident
            .card
            .vars()
            .iter()
            .map(|var| Printed {
                id: var.id,
                // A build can store exact fractions where this slice only
                // evaluates whole-number effects.
                base: num_traits::NumCast::from(var.base).unwrap_or_else(|| {
                    unimplemented!("the fractional {} this card prints", var.id.name())
                }),
                calculated: var.calculated,
            })
            .collect();
        Self { ident, vars }
    }
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
    pub(super) encounter: Option<V::Encounter>,
    pub(super) hand: Vec<CardInstance<V>>,
    pub(super) draw: Vec<CardInstance<V>>,
    pub(super) discard: Vec<CardInstance<V>>,
    pub(super) exhaust: Vec<CardInstance<V>>,
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
}

impl<V: Rules> Live<V> {
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
            .position(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER);
        for creature in &mut creatures {
            let (Some(player), Some(next)) = (player, &mut creature.next_move) else {
                continue;
            };
            for intent in &mut next.intents {
                if let Some(attack) = &mut intent.attack {
                    attack.damage = super::board::modified::<V>(
                        &self.creatures,
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
            encounter: self.encounter,
            hand: self.hand.iter().map(|card| card.ident).collect(),
            draw: self.draw.iter().map(|card| card.ident).collect(),
            discard: self.discard.iter().map(|card| card.ident).collect(),
            exhaust: self.exhaust.iter().map(|card| card.ident).collect(),
            creatures,
            legal: super::legal::hand::<V>(&self.hand, &self.creatures, self.energy, self.stars),
            orbs: self.orbs.clone(),
        }
    }
}

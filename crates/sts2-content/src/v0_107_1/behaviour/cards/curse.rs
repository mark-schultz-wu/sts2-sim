//! The curses the generator leaves to a hand port.

use sts2_core::{Amount, Effect, Fight, PowerAmount, Scaling};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardVar, CurseCard};
use super::super::super::generated::powers::Power;
use super::V;

/// `Shame`'s `DynamicVar("Frail", 1)`.
const SHAME_FRAIL: super::Number = 1;

/// A curse's effect while it remains in the hand at turn end.
pub(super) fn on_turn_end<F: Fight<V>>(card: CurseCard, fight: &mut F) {
    match card {
        CurseCard::Shame => shame(fight),
        CurseCard::Debt => debt(fight),
        CurseCard::Decay => decay(fight),
        card => card_plays::curse::on_turn_end(card, fight),
    }
}

/// `Shame`: its Frail onto its holder, which then goes to the discard pile.
/// Frail landing on the player skips its next tick, which is what Shame sets
/// on a fresh one.
fn shame<F: Fight<V>>(fight: &mut F) {
    fight.apply_to_actor(Power::FrailPower, PowerAmount::Counted(SHAME_FRAIL));
    fight.discard_this();
}

/// `Debt`: `PlayerCmd.LoseGold` of its `Gold`, or of what its owner holds
/// where that is less. Then it goes to the discard pile.
fn debt<F: Fight<V>>(fight: &mut F) {
    fight.lose_gold(fight.var(CardVar::Gold));
    fight.discard_this();
}

/// `Decay`: its damage to its holder, unpowered, and then it goes to the
/// discard pile, as every card with a turn-end effect does.
fn decay<F: Fight<V>>(fight: &mut F) {
    let damage = fight.resolve(Effect::Attack {
        dealer: fight.actor(),
        target: fight.actor(),
        damage: Amount::Var(CardVar::Damage),
        scaling: Scaling::Static,
    });
    fight.apply(damage);
    fight.discard_this();
}

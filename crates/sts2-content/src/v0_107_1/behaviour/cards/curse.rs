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
        CurseCard::Doubt => doubt(fight),
        CurseCard::Regret => regret(fight),
        CurseCard::BadLuck => bad_luck(fight),
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

/// `Doubt`: its Weak onto its holder, which then goes to the discard pile.
/// Weak landing on the player skips its next tick, as Shame's Frail does.
fn doubt<F: Fight<V>>(fight: &mut F) {
    fight.apply_to_actor(
        Power::WeakPower,
        PowerAmount::Counted(fight.var(CardVar::WeakPower)),
    );
    fight.discard_this();
}

/// `Regret`: as many hit points off its holder, unblockable, as the hand
/// held as the turn began to end, which it noted then. Then it keeps nothing
/// and goes to the discard pile.
fn regret<F: Fight<V>>(fight: &mut F) {
    fight.damage_unblockable(fight.actor(), fight.self_note());
    fight.set_self_note(0);
    fight.discard_this();
}

/// `BadLuck`: its `HpLoss` off its holder, unblockable, and then it goes to
/// the discard pile.
fn bad_luck<F: Fight<V>>(fight: &mut F) {
    fight.damage_unblockable(fight.actor(), fight.var(CardVar::HpLoss));
    fight.discard_this();
}

/// Deliver a notification to one curse in the fight.
pub(super) fn event_hook(
    card: CurseCard,
    pile: Option<sts2_core::Pile>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    use super::super::super::generated::runstate::CombatSide;
    use sts2_core::{EventHook, HookEffect, Pile};

    match (card, hook) {
        // `Regret.BeforeSideTurnEnd`: the hand's size noted as `CardsInHand`,
        // where it stands in the hand as its owner's turn ends.
        (
            CurseCard::Regret,
            EventHook::BeforeSideTurnEnd {
                side: CombatSide::Player,
                hand,
                ..
            },
        ) => {
            if pile == Some(Pile::Hand) {
                let hand = super::Number::try_from(hand).expect("a hand fits a game number");
                asked.push(HookEffect::NoteOnSelf(hand));
            }
        }
        (CurseCard::Regret, EventHook::BeforeSideTurnEnd { .. }) => {}
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
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

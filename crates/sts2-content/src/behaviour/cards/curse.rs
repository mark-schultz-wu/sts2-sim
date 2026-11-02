//! The curses the generator leaves to a hand port.

use sts2_core::{Amount, Effect, Fight, PowerAmount, Scaling};

use crate::ids::{CardIds as _, CardVarIds as _, PowerIds as _};

/// `Shame`'s `DynamicVar("Frail", 1)`.
const SHAME_FRAIL: crate::behaviour::cards::Number = 1;

/// A curse's effect while it remains in the hand at turn end.
pub(super) fn on_turn_end<V: crate::Build, F: Fight<V>>(card: V::Card, fight: &mut F) {
    match card {
        __c0 if __c0 == V::Card::SHAME => shame::<V, _>(fight),
        __c0 if __c0 == V::Card::DOUBT => doubt::<V, _>(fight),
        __c0 if __c0 == V::Card::REGRET => regret::<V, _>(fight),
        __c0 if __c0 == V::Card::BAD_LUCK => bad_luck::<V, _>(fight),
        __c0 if __c0 == V::Card::DEBT => debt::<V, _>(fight),
        __c0 if __c0 == V::Card::DECAY => decay::<V, _>(fight),
        card => V::generated_turn_end(card, fight),
    }
}

/// `Shame`: its Frail onto its holder.
/// Frail landing on the player skips its next tick, which is what Shame sets
/// on a fresh one.
fn shame<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.apply_to_actor(V::Power::FRAIL_POWER, PowerAmount::Counted(SHAME_FRAIL));
}

/// `Doubt`: its Weak onto its holder.
/// Weak landing on the player skips its next tick, as Shame's Frail does.
fn doubt<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.apply_to_actor(
        V::Power::WEAK_POWER,
        PowerAmount::Counted(fight.var(V::CardVar::WEAK_POWER)),
    );
}

/// `Regret`: as many hit points off its holder, unblockable, as the hand
/// held as the turn began to end, which it noted then. Then it keeps nothing.
fn regret<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.damage_unblockable(fight.actor(), fight.self_note());
    fight.set_self_note(0);
}

/// `BadLuck`: its `HpLoss` off its holder, unblockable.
fn bad_luck<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.damage_unblockable(fight.actor(), fight.var(V::CardVar::HP_LOSS));
}

/// Deliver a notification to one curse in the fight.
pub(super) fn event_hook<V: crate::Build>(
    card: V::Card,
    owner: sts2_core::CombatId,
    pile: Option<sts2_core::Pile>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    use sts2_core::{EventHook, HookEffect, Pile};

    match (card, hook) {
        // `Regret.BeforeSideTurnEnd`: the hand's size noted as `CardsInHand`,
        // where it stands in the hand and its owner takes part in the turn's
        // end.
        (
            __c0,
            EventHook::BeforeSideTurnEnd {
                participants, hand, ..
            },
        ) if __c0 == V::Card::REGRET && participants.contains(owner) => {
            if pile == Some(Pile::Hand) {
                let hand = crate::behaviour::cards::Number::try_from(hand)
                    .expect("a hand fits a game number");
                asked.push(HookEffect::NoteOnSelf(hand));
            }
        }
        (__c0, EventHook::BeforeSideTurnEnd { .. }) if __c0 == V::Card::REGRET => {}
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}

/// `Debt`: `PlayerCmd.LoseGold` of its `Gold`, or of what its owner holds
/// where that is less.
fn debt<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.lose_gold(fight.var(V::CardVar::GOLD));
}

/// `Decay`: its damage to its holder, unpowered.
fn decay<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let damage = fight.resolve(Effect::Attack {
        dealer: fight.actor(),
        target: fight.actor(),
        damage: Amount::Var(V::CardVar::DAMAGE),
        scaling: Scaling::Static,
    });
    fight.apply(damage);
}

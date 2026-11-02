//! The cards a fight puts in a deck that nobody chose.

use sts2_core::{Amount, Effect, EventHook, Fight, HookEffect, Scaling};

use crate::ids::{CardIds as _, CardVarIds as _, PowerIds as _};

/// What a status card does when it is played.
pub(super) fn on_play<'a, V: crate::Build, K: sts2_core::FightKind<V>>(
    card: V::Card,
    fight: &mut K::Fight<'a>,
) -> crate::behaviour::Played<V, K> {
    match card {
        __c0 if __c0 == V::Card::FRANTIC_ESCAPE => frantic_escape::<V, _>(fight),
        card => crate::behaviour::cards::generated::<V, K>(card, fight),
    }
}

/// `FranticEscape`: one more on the first enemy's Sandpit that is aimed at
/// the card's owner. Then its own cost is one higher for the rest of the
/// fight.
fn frantic_escape<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
) -> crate::behaviour::Played<V, F::Kind> {
    use sts2_core::CreatureView as _;

    let (owner, player_side) = (fight.actor(), fight.side());
    let pit = fight
        .creatures()
        .find(|creature| {
            creature.side() != player_side && creature.power(V::Power::SANDPIT_POWER).is_some()
        })
        .map(|creature| creature.id());
    if let Some(pit) = pit
        && let Some(nth) = fight
            .power_instances(pit, V::Power::SANDPIT_POWER)
            .iter()
            .position(|&(target, _)| target == Some(owner))
    {
        fight.modify_power_at(pit, V::Power::SANDPIT_POWER, nth, 1, Some(pit));
    }
    fight.modify_this(
        sts2_core::Change::ChangeCost(sts2_core::CostPool::Energy, 1),
        sts2_core::Lasting::ThisFight,
    );
    crate::behaviour::Played::Done
}

/// A status card's effect while it remains in the hand at turn end.
pub(super) fn on_turn_end<V: crate::Build, F: Fight<V>>(card: V::Card, fight: &mut F) {
    match card {
        __c0 if __c0 == V::Card::INFECTION
            || __c0 == V::Card::BURN
            || __c0 == V::Card::WITHER
            || __c0 == V::Card::TOXIC =>
        {
            infection::<V, _>(fight)
        }
        // `Beckon`: its `HpLoss` off its holder, unblockable.
        __c0 if __c0 == V::Card::BECKON => {
            fight.damage_unblockable(fight.actor(), fight.var(V::CardVar::HP_LOSS));
        }
        card => V::generated_turn_end(card, fight),
    }
}

/// `Infection`, `Burn`, `Wither` and `Toxic`, each of which hurts its holder
/// by its damage.
fn infection<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let damage = fight.resolve(Effect::Attack {
        dealer: fight.actor(),
        target: fight.actor(),
        damage: Amount::Var(V::CardVar::DAMAGE),
        scaling: Scaling::Static,
    });
    fight.apply(damage);
}

/// Deliver a notification to one status card in the fight.
pub(super) fn event_hook<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    card: V::Card,
    id: sts2_core::CardId<V>,
    printed: &P,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // `Void.AfterCardDrawn`, which acts on the draw that brought it here and
    // on no other card's.
    match (card, hook) {
        (__c0, EventHook::CardDrawn { id: drawn, .. }) if __c0 == V::Card::VOID => {
            if drawn == id {
                asked.push(HookEffect::LoseEnergy(printed.face(V::CardVar::ENERGY)));
            }
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}
